import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import { Check, Flag, MinusCircle, Settings, SkipForward, Volume2 } from "lucide-react";
import renderMathInElement from "katex/contrib/auto-render";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { DeckOptionsDialog } from "@/components/DeckOptionsDialog";
import { DeckCounts } from "@/components/decks/DeckCounts";
import { ExtendTodayLimit } from "@/components/decks/IncreaseLimitControl";
import { ipc, isTauri, Rating, type RatingValue } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { mediaUrl, prepareCard } from "@/lib/renderCard";
import { dur, ease, listItem, staggerList, useReducedMotion } from "@/lib/motion";

const FLAG_COLORS: Record<number, string> = {
  0: "text-muted-foreground",
  1: "text-red-500",
  2: "text-orange-500",
  3: "text-green-500",
  4: "text-blue-500",
};

const KATEX_OPTIONS = {
  delimiters: [
    { left: "\\(", right: "\\)", display: false },
    { left: "\\[", right: "\\]", display: true },
    { left: "$$", right: "$$", display: true },
    { left: "$", right: "$", display: false },
  ],
  throwOnError: false,
} as const;

export function StudySessionScreen() {
  const { deckId } = useParams({ from: "/study/$deckId" });
  const navigate = useNavigate();
  return <Session deckId={deckId} onExit={() => void navigate({ to: "/" })} />;
}

function Session({ deckId, onExit }: { deckId: number; onExit: () => void }) {
  const tauri = isTauri();
  const queryClient = useQueryClient();
  const prefersReduced = useReducedMotion();
  const [revealed, setRevealed] = useState(false);
  const [flagMenuOpen, setFlagMenuOpen] = useState(false);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const flagRef = useRef<HTMLDivElement>(null);

  const deckName = useQuery({
    queryKey: queryKeys.decks,
    queryFn: ipc.listDecks,
    enabled: tauri,
    select: (decks) => decks.find((d) => d.id === deckId)?.name,
  }).data;

  const cardQuery = useQuery({
    queryKey: queryKeys.queue(String(deckId)),
    queryFn: () => ipc.getNextCard(deckId),
    refetchInterval: (query) => (query.state.data == null ? 15000 : false),
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: false,
  });

  const answerMut = useMutation({
    mutationFn: ({ cardId, rating }: { cardId: number; rating: RatingValue }) =>
      ipc.answerCard(cardId, rating, deckId),
    onSuccess: (next) => {
      queryClient.setQueryData(queryKeys.queue(String(deckId)), next ?? null);
      setRevealed(false);
    },
  });

  const suspendMut = useMutation({
    mutationFn: (cardId: number) => ipc.suspendCards([cardId]),
    onSuccess: () =>
      void queryClient
        .refetchQueries({ queryKey: queryKeys.queue(String(deckId)) })
        .then(() => setRevealed(false)),
  });

  const buryMut = useMutation({
    mutationFn: (cardId: number) => ipc.buryCards([cardId]),
    onSuccess: () =>
      void queryClient
        .refetchQueries({ queryKey: queryKeys.queue(String(deckId)) })
        .then(() => setRevealed(false)),
  });

  const flagMut = useMutation({
    mutationFn: ({ cardId, flag }: { cardId: number; flag: number }) =>
      ipc.setCardFlag([cardId], flag),
    onSuccess: () => setFlagMenuOpen(false),
  });

  const card = cardQuery.data ?? null;
  const actionBusy = suspendMut.isPending || buryMut.isPending || flagMut.isPending;

  const prepared = useMemo(
    () =>
      card
        ? {
            q: prepareCard(card.question, tauri),
            a: prepareCard(card.answer, tauri),
          }
        : null,
    [card, tauri],
  );

  const currentSounds = useMemo(
    () => (prepared ? (revealed ? prepared.a.sounds : prepared.q.sounds) : []),
    [prepared, revealed],
  );

  // Audio sequencer
  const [soundIdx, setSoundIdx] = useState(-1);
  const cardKey = card?.card_id ?? -1;

  useEffect(() => {
    setSoundIdx(currentSounds.length > 0 ? 0 : -1);
  }, [cardKey, revealed]); // eslint-disable-line react-hooks/exhaustive-deps

  const replayAudio = useCallback(() => {
    if (currentSounds.length > 0) setSoundIdx(0);
  }, [currentSounds.length]);

  useEffect(() => {
    if (!tauri || soundIdx < 0 || soundIdx >= currentSounds.length) return;
    const audio = new Audio(mediaUrl(currentSounds[soundIdx]));
    audio.play().catch(() => {});
    audio.onended = () => setSoundIdx((i) => i + 1);
    return () => {
      audio.pause();
      audio.src = "";
    };
  }, [tauri, soundIdx, currentSounds]);

  // KaTeX: run on both flip faces when the card changes.
  const cardFrontRef = useRef<HTMLDivElement>(null);
  const cardBackRef = useRef<HTMLDivElement>(null);
  // Fallback ref for reduced-motion path.
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (prefersReduced) {
      if (cardRef.current) renderMathInElement(cardRef.current, KATEX_OPTIONS);
    } else {
      if (cardFrontRef.current) renderMathInElement(cardFrontRef.current, KATEX_OPTIONS);
      if (cardBackRef.current) renderMathInElement(cardBackRef.current, KATEX_OPTIONS);
    }
  }, [card?.card_id, prefersReduced]);

  // Also re-run KaTeX on reduced-motion path when revealed (html changes).
  const currentHtml = prepared ? (revealed ? prepared.a.html : prepared.q.html) : "";
  useEffect(() => {
    if (!prefersReduced || !cardRef.current) return;
    renderMathInElement(cardRef.current, KATEX_OPTIONS);
  }, [currentHtml, prefersReduced]);

  // Close flag menu on outside click.
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (flagRef.current && !flagRef.current.contains(e.target as Node)) {
        setFlagMenuOpen(false);
      }
    };
    if (flagMenuOpen) document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [flagMenuOpen]);

  // Keyboard shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!card || answerMut.isPending || actionBusy) return;
      if (e.key === "r" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        replayAudio();
        return;
      }
      if (!revealed && (e.key === " " || e.key === "Enter")) {
        e.preventDefault();
        setRevealed(true);
        return;
      }
      if (revealed && ["1", "2", "3", "4"].includes(e.key)) {
        e.preventDefault();
        const btn = getAnswerButtons(card).find((b) => b.hotkey === e.key);
        if (btn) answerMut.mutate({ cardId: card.card_id, rating: btn.rating });
        return;
      }
      if (e.key === "s" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        suspendMut.mutate(card.card_id);
      }
      if (e.key === "b" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        buryMut.mutate(card.card_id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [card, revealed, answerMut, actionBusy, suspendMut, buryMut, replayAudio]);

  const flipDuration = prefersReduced ? 0 : dur.slow;
  const fadeDuration = prefersReduced ? 0 : dur.base;

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        title={deckName ? `Studying · ${deckName}` : "Studying"}
        actions={
          <div className="flex items-center gap-3">
            {card && (
              <DeckCounts
                newCount={card.new_count}
                learningCount={card.learning_count}
                reviewCount={card.review_count}
              />
            )}
            <Button variant="ghost" size="sm" onClick={onExit}>
              Back to decks
            </Button>
          </div>
        }
      />
      {card ? (
        <motion.div
          key={card.card_id}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: fadeDuration, ease }}
          className="mx-auto flex w-full max-w-2xl flex-1 flex-col px-6 py-8"
        >
          {/* Card — 3D flip (or plain div for reduced-motion) */}
          {prefersReduced ? (
            <div
              ref={cardRef}
              key={card.card_id + (revealed ? "a" : "q")}
              className="synapse-card flex-1 rounded-xl border border-border bg-card p-8 text-card-foreground overflow-auto"
              dangerouslySetInnerHTML={{ __html: currentHtml }}
            />
          ) : (
            <div className="synapse-flip relative flex-1">
              <motion.div
                className="absolute inset-0"
                style={{ transformStyle: "preserve-3d" }}
                animate={{ rotateY: revealed ? 180 : 0 }}
                transition={{ duration: flipDuration, ease }}
              >
                {/* Front — question */}
                <div
                  ref={cardFrontRef}
                  className="synapse-card absolute inset-0 rounded-xl border border-border bg-card p-8 text-card-foreground overflow-auto"
                  style={{ backfaceVisibility: "hidden" }}
                  dangerouslySetInnerHTML={{ __html: prepared?.q.html ?? "" }}
                />
                {/* Back — answer (pre-rotated so it faces forward when parent is at 180°) */}
                <div
                  ref={cardBackRef}
                  className="synapse-card absolute inset-0 rounded-xl border border-border bg-card p-8 text-card-foreground overflow-auto"
                  style={{ backfaceVisibility: "hidden", transform: "rotateY(180deg)" }}
                  dangerouslySetInnerHTML={{ __html: prepared?.a.html ?? "" }}
                />
              </motion.div>
            </div>
          )}

          <div className="mt-6">
            {/* Answer buttons with stagger, or Show Answer button */}
            <AnimatePresence mode="wait" initial={false}>
              {revealed ? (
                <motion.div
                  key="answers"
                  className={`grid gap-2 ${getAnswerButtons(card).length === 3 ? "grid-cols-3" : "grid-cols-4"}`}
                  variants={staggerList}
                  initial="hidden"
                  animate="show"
                >
                  {getAnswerButtons(card).map((btn) => (
                    <motion.div key={btn.label} variants={listItem}>
                      <AnswerButton
                        label={btn.label}
                        hint={btn.hint}
                        hotkey={btn.hotkey}
                        variant={btn.variant}
                        onClick={() =>
                          answerMut.mutate({ cardId: card.card_id, rating: btn.rating })
                        }
                      />
                    </motion.div>
                  ))}
                </motion.div>
              ) : (
                <motion.div
                  key="reveal"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: fadeDuration }}
                >
                  <Button className="w-full" onClick={() => setRevealed(true)}>
                    Show answer <span className="ml-2 text-xs opacity-70">Space</span>
                  </Button>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Card actions: audio / suspend / bury / flag */}
            <div className="mt-3 flex items-center justify-end gap-1">
              {currentSounds.length > 0 && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 gap-1.5 px-2 text-xs text-muted-foreground"
                  onClick={replayAudio}
                  title="Replay audio (r)"
                >
                  <Volume2 className="size-3.5" />
                  Replay
                </Button>
              )}
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1.5 px-2 text-xs text-muted-foreground"
                disabled={actionBusy}
                onClick={() => suspendMut.mutate(card.card_id)}
                title="Suspend card (s)"
              >
                <MinusCircle className="size-3.5" />
                Suspend
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1.5 px-2 text-xs text-muted-foreground"
                disabled={actionBusy}
                onClick={() => buryMut.mutate(card.card_id)}
                title="Bury card (b)"
              >
                <SkipForward className="size-3.5" />
                Bury
              </Button>
              <div className="relative" ref={flagRef}>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 gap-1.5 px-2 text-xs"
                  disabled={actionBusy}
                  onClick={() => setFlagMenuOpen((o) => !o)}
                  title="Set flag"
                >
                  <Flag className="size-3.5 text-muted-foreground" />
                  <span className="text-muted-foreground">Flag</span>
                </Button>
                <AnimatePresence>
                  {flagMenuOpen && (
                    <motion.div
                      initial={{ opacity: 0, scale: 0.95, y: 4 }}
                      animate={{ opacity: 1, scale: 1, y: 0 }}
                      exit={{ opacity: 0, scale: 0.95, y: 4 }}
                      transition={{ duration: dur.fast, ease }}
                      role="menu"
                      aria-label="Set flag"
                      className="absolute bottom-full right-0 mb-1 flex overflow-hidden rounded-lg border border-border bg-popover shadow-md"
                    >
                      {[0, 1, 2, 3, 4].map((f) => (
                        <button
                          key={f}
                          role="menuitem"
                          aria-label={f === 0 ? "Remove flag" : `Flag ${f}`}
                          className={`p-2 hover:bg-accent ${FLAG_COLORS[f]}`}
                          onClick={() => flagMut.mutate({ cardId: card.card_id, flag: f })}
                        >
                          <Flag className="size-4" />
                        </button>
                      ))}
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </div>
          </div>
        </motion.div>
      ) : (
        <EmptyState
          icon={Check}
          title="All done"
          description="No more cards due in this deck right now. Nice work."
          action={
            <div className="flex flex-col items-center gap-3">
              <Button variant="outline" onClick={onExit}>
                Back to decks
              </Button>
              <ExtendTodayLimit
                deckId={deckId}
                onDone={() =>
                  void queryClient.refetchQueries({ queryKey: queryKeys.queue(String(deckId)) })
                }
              />
              <Button
                variant="ghost"
                size="sm"
                className="gap-1.5 text-xs text-muted-foreground"
                onClick={() => setOptionsOpen(true)}
              >
                <Settings className="size-3.5" />
                Deck options
              </Button>
            </div>
          }
        />
      )}

      <AnimatePresence>
        {optionsOpen && deckName && (
          <DeckOptionsDialog
            deckId={deckId}
            deckName={deckName}
            onClose={() => setOptionsOpen(false)}
            onSaved={() =>
              void queryClient.invalidateQueries({ queryKey: queryKeys.queue(String(deckId)) })
            }
          />
        )}
      </AnimatePresence>
    </div>
  );
}

type AnswerBtnDef = {
  label: string;
  hint: string;
  hotkey: string;
  variant: "default" | "secondary" | "outline" | "destructive";
  rating: RatingValue;
};

function getAnswerButtons(card: {
  again: string;
  hard: string;
  good: string;
  easy: string;
  algorithm: string;
  card_phase: string;
}): AnswerBtnDef[] {
  const isFsrsReview =
    card.algorithm === "fsrs" && (card.card_phase === "review" || card.card_phase === "relearning");

  if (isFsrsReview) {
    return [
      {
        label: "Forgot",
        hint: card.again,
        hotkey: "1",
        variant: "destructive",
        rating: Rating.Again,
      },
      {
        label: "Remembered",
        hint: card.good,
        hotkey: "2",
        variant: "default",
        rating: Rating.Good,
      },
      { label: "Easy", hint: card.easy, hotkey: "3", variant: "outline", rating: Rating.Easy },
    ];
  }

  const isFsrs = card.algorithm === "fsrs";
  return [
    {
      label: isFsrs ? "Forgot" : "Again",
      hint: card.again,
      hotkey: "1",
      variant: "destructive",
      rating: Rating.Again,
    },
    { label: "Hard", hint: card.hard, hotkey: "2", variant: "secondary", rating: Rating.Hard },
    {
      label: isFsrs ? "Remembered" : "Good",
      hint: card.good,
      hotkey: "3",
      variant: "default",
      rating: Rating.Good,
    },
    { label: "Easy", hint: card.easy, hotkey: "4", variant: "outline", rating: Rating.Easy },
  ];
}

function AnswerButton(props: {
  label: string;
  hint: string;
  hotkey: string;
  variant: "default" | "secondary" | "outline" | "destructive";
  onClick: () => void;
}) {
  return (
    <motion.div whileTap={{ scale: 0.97 }} className="w-full">
      <Button
        variant={props.variant}
        className="h-auto w-full flex-col gap-0.5 py-2"
        onClick={props.onClick}
      >
        <span className="text-xs opacity-70">{props.hint}</span>
        <span>
          {props.label} <span className="opacity-60">{props.hotkey}</span>
        </span>
      </Button>
    </motion.div>
  );
}
