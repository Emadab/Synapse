import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import {
  Check,
  Flag,
  Maximize2,
  Minimize2,
  MinusCircle,
  Settings,
  SkipForward,
  Volume2,
} from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { CardFace } from "@/components/CardFace";
import { DeckOptionsDialog } from "@/components/DeckOptionsDialog";
import { DeckCounts } from "@/components/decks/DeckCounts";
import { ExtendTodayLimit } from "@/components/decks/IncreaseLimitControl";
import { Kbd } from "@/components/Kbd";
import { ipc, isTauri, Rating, type RatingValue } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { mediaUrl, type QueueEntry } from "@/lib/renderCard";
import { dur, ease, listItem, staggerList, useReducedMotion } from "@/lib/motion";
import { useTheme } from "@/stores/theme";
import { useUi } from "@/stores/ui";
import { speak, cancelSpeech } from "@/lib/tts";

const FLAG_COLORS: Record<number, string> = {
  1: "text-red-500",
  2: "text-orange-500",
  3: "text-green-500",
  4: "text-blue-500",
};

export function StudySessionScreen() {
  const { deckId } = useParams({ from: "/study/$deckId" });
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const onExit = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.decks });
    void navigate({ to: "/" });
  };
  return <Session deckId={deckId} onExit={onExit} />;
}

function Session({ deckId, onExit }: { deckId: number; onExit: () => void }) {
  const tauri = isTauri();
  const queryClient = useQueryClient();
  const prefersReduced = useReducedMotion();
  const [revealed, setRevealed] = useState(false);
  const [flagMenuOpen, setFlagMenuOpen] = useState(false);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [sessionAnswered, setSessionAnswered] = useState(0);
  const flagRef = useRef<HTMLDivElement>(null);
  const focusMode = useUi((s) => s.focusMode);
  const toggleFocusMode = useUi((s) => s.toggleFocusMode);
  const setFocusMode = useUi((s) => s.setFocusMode);

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
      ipc.answerCard(cardId, rating, deckId, Date.now() - shownAtRef.current),
    onSuccess: (next) => {
      queryClient.setQueryData(queryKeys.queue(String(deckId)), next ?? null);
      setRevealed(false);
      setSessionAnswered((n) => n + 1);
    },
  });

  const [actionToast, setActionToast] = useState<{ message: string; onUndo?: () => void } | null>(
    null,
  );
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const showActionToast = useCallback((message: string, onUndo?: () => void) => {
    clearTimeout(toastTimerRef.current);
    setActionToast({ message, onUndo });
    toastTimerRef.current = setTimeout(() => setActionToast(null), 3000);
  }, []);

  const unsuspendMut = useMutation({
    mutationFn: (cardId: number) => ipc.unsuspendCards([cardId]),
    onSuccess: () => {
      setActionToast(null);
      void queryClient.refetchQueries({ queryKey: queryKeys.queue(String(deckId)) });
    },
  });

  const suspendMut = useMutation({
    mutationFn: (cardId: number) => ipc.suspendCards([cardId]),
    onSuccess: (_data, cardId) => {
      showActionToast("Card suspended", () => unsuspendMut.mutate(cardId));
      void queryClient
        .refetchQueries({ queryKey: queryKeys.queue(String(deckId)) })
        .then(() => setRevealed(false));
    },
  });

  const buryMut = useMutation({
    mutationFn: (cardId: number) => ipc.buryCards([cardId]),
    onSuccess: () => {
      // No per-card unbury IPC command exists yet (only whole-deck unbury) —
      // confirm the action without offering undo until one is added.
      showActionToast("Card buried");
      void queryClient
        .refetchQueries({ queryKey: queryKeys.queue(String(deckId)) })
        .then(() => setRevealed(false));
    },
  });

  const flagMut = useMutation({
    mutationFn: ({ cardId, flag }: { cardId: number; flag: number }) =>
      ipc.setCardFlag([cardId], flag),
    onSuccess: () => setFlagMenuOpen(false),
  });

  const card = cardQuery.data ?? null;
  const actionBusy = suspendMut.isPending || buryMut.isPending || flagMut.isPending;

  const night = useTheme((s) => s.resolved === "dark");
  const [frontQueue, setFrontQueue] = useState<QueueEntry[]>([]);
  const [backQueue, setBackQueue] = useState<QueueEntry[]>([]);
  const [typedAnswer, setTypedAnswer] = useState("");
  const currentQueue = revealed ? backQueue : frontQueue;

  // Sound + TTS sequencer — plays [sound:...] files and {{tts:}} utterances
  // in the document order the render engine's unified queue reports.
  const [queueIdx, setQueueIdx] = useState(-1);
  const cardKey = card?.card_id ?? -1;
  const shownAtRef = useRef(Date.now());

  useEffect(() => {
    setTypedAnswer("");
    shownAtRef.current = Date.now();
  }, [cardKey]);

  useEffect(() => {
    setQueueIdx(currentQueue.length > 0 ? 0 : -1);
  }, [cardKey, revealed]); // eslint-disable-line react-hooks/exhaustive-deps

  const replayAudio = useCallback(() => {
    if (currentQueue.length > 0) setQueueIdx(0);
  }, [currentQueue.length]);

  useEffect(() => {
    if (queueIdx < 0 || queueIdx >= currentQueue.length) return;
    const entry = currentQueue[queueIdx];
    const advance = () => setQueueIdx((i) => i + 1);

    if (entry.kind === "file") {
      if (!tauri) {
        advance();
        return;
      }
      const audio = new Audio(mediaUrl(entry.name));
      audio.play().catch(() => {});
      audio.onended = advance;
      return () => {
        audio.pause();
        audio.src = "";
      };
    }

    let cancelled = false;
    void speak(entry.text, { lang: entry.lang, voices: entry.voices, rate: entry.rate }).then(
      () => {
        if (!cancelled) advance();
      },
    );
    return () => {
      cancelled = true;
      cancelSpeech();
    };
  }, [tauri, queueIdx, currentQueue]);

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
      const typing =
        e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;

      // Esc exits focus mode regardless of typing state; otherwise ignore
      // shortcuts while the user is typing (e.g. into the type-answer field).
      if (e.key === "Escape") {
        if (focusMode) {
          e.preventDefault();
          setFocusMode(false);
        }
        return;
      }
      if (typing) return;

      if (e.key === "f" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        toggleFocusMode();
        return;
      }
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
  }, [
    card,
    revealed,
    answerMut,
    actionBusy,
    suspendMut,
    buryMut,
    replayAudio,
    focusMode,
    setFocusMode,
    toggleFocusMode,
  ]);

  const flipDuration = prefersReduced ? 0 : dur.slow;
  const fadeDuration = prefersReduced ? 0 : dur.base;

  const sessionTotal = card
    ? sessionAnswered + card.new_count + card.learning_count + card.review_count
    : sessionAnswered;
  const progressFraction = sessionTotal > 0 ? sessionAnswered / sessionTotal : 0;

  return (
    <div className="relative flex h-full flex-col">
      {focusMode ? (
        <div className="glass-panel relative z-20 flex h-9 shrink-0 items-center justify-between gap-3 border-b px-4">
          <div
            className="absolute inset-x-0 bottom-0 h-px bg-primary/60 transition-[width] duration-300"
            style={{ width: `${Math.round(progressFraction * 100)}%` }}
          />
          <span className="truncate text-[13px] text-muted-foreground">{deckName}</span>
          <div className="flex items-center gap-3">
            {card && (
              <DeckCounts
                newCount={card.new_count}
                learningCount={card.learning_count}
                reviewCount={card.review_count}
              />
            )}
            <button
              type="button"
              onClick={() => setFocusMode(false)}
              title="Exit focus (Esc)"
              className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              <Minimize2 className="size-3.5" />
            </button>
          </div>
        </div>
      ) : (
        <ScreenHeader
          title={deckName ?? "Studying"}
          actions={
            <div className="flex items-center gap-3">
              {card && (
                <DeckCounts
                  newCount={card.new_count}
                  learningCount={card.learning_count}
                  reviewCount={card.review_count}
                />
              )}
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setFocusMode(true)}
                title="Focus mode (F)"
              >
                <Maximize2 className="size-3.5" />
              </Button>
              <Button variant="ghost" size="sm" onClick={onExit}>
                Back to decks
              </Button>
            </div>
          }
        />
      )}

      {focusMode && (
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 z-0"
          style={{
            background:
              "radial-gradient(ellipse at center, transparent 45%, hsl(var(--background)) 100%)",
          }}
        />
      )}

      {card ? (
        <motion.div
          key={card.card_id}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: fadeDuration, ease }}
          className={`relative z-10 mx-auto flex w-full min-h-0 flex-1 flex-col px-3 py-4 sm:px-6 sm:py-8 ${focusMode ? "max-w-3xl" : "max-w-2xl"}`}
        >
          {/* Card — 3D flip (or plain div for reduced-motion) */}
          {prefersReduced ? (
            <CardFace
              key={card.card_id}
              html={revealed ? card.answer : card.question}
              css={card.css}
              tauri={tauri}
              night={night}
              className="synapse-card min-h-0 flex-1 rounded-xl border border-border p-4 sm:p-8 overflow-auto"
              onQueue={revealed ? setBackQueue : setFrontQueue}
              onTypedInput={!revealed ? setTypedAnswer : undefined}
              typedAnswer={revealed ? typedAnswer : undefined}
            />
          ) : (
            <div className="synapse-flip relative min-h-0 flex-1">
              <motion.div
                className="absolute inset-0"
                style={{ transformStyle: "preserve-3d" }}
                animate={{ rotateY: revealed ? 180 : 0 }}
                transition={{ duration: flipDuration, ease }}
              >
                {/* Front — question */}
                <CardFace
                  html={card.question}
                  css={card.css}
                  tauri={tauri}
                  night={night}
                  className="synapse-card absolute inset-0 rounded-xl border border-border p-4 sm:p-8 overflow-auto"
                  style={{ backfaceVisibility: "hidden" }}
                  onQueue={setFrontQueue}
                  onTypedInput={setTypedAnswer}
                />
                {/* Back — answer (pre-rotated so it faces forward when parent is at 180°) */}
                <CardFace
                  html={card.answer}
                  css={card.css}
                  tauri={tauri}
                  night={night}
                  className="synapse-card absolute inset-0 rounded-xl border border-border p-4 sm:p-8 overflow-auto"
                  style={{ backfaceVisibility: "hidden", transform: "rotateY(180deg)" }}
                  onQueue={setBackQueue}
                  typedAnswer={typedAnswer}
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
                    Show answer
                    <Kbd className="ml-2 border-primary-foreground/20 bg-primary-foreground/10 text-primary-foreground/80">
                      Space
                    </Kbd>
                  </Button>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Card actions: audio / suspend / bury / flag */}
            <div className="mt-3 flex items-center justify-end gap-1">
              {currentQueue.length > 0 && (
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
                  onContextMenu={(e) => {
                    e.preventDefault();
                    flagMut.mutate({ cardId: card.card_id, flag: 0 });
                  }}
                  title="Set flag (right-click to clear)"
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
                      className="glass-panel absolute bottom-full right-0 mb-1 flex overflow-hidden rounded-lg border shadow-md"
                    >
                      {[1, 2, 3, 4].map((f) => (
                        <button
                          key={f}
                          role="menuitem"
                          aria-label={`Flag ${f}`}
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

            {!focusMode && (
              <div className="mt-3 flex items-center justify-center gap-1.5 text-[11px] text-muted-foreground/70">
                <Kbd>Space</Kbd> reveal
                <span className="mx-1">·</span>
                <Kbd>1–4</Kbd> rate
                <span className="mx-1">·</span>
                <Kbd>F</Kbd> focus
              </div>
            )}
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

      <AnimatePresence>
        {actionToast && (
          <motion.div
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            transition={{ duration: dur.fast, ease }}
            className="glass-panel absolute bottom-4 left-1/2 z-30 flex -translate-x-1/2 items-center gap-3 rounded-lg border px-3 py-2 text-sm shadow-md"
          >
            <span>{actionToast.message}</span>
            {actionToast.onUndo && (
              <button
                type="button"
                className="font-medium text-primary hover:underline"
                onClick={actionToast.onUndo}
              >
                Undo
              </button>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

type AnswerBtnDef = {
  label: string;
  hint: string;
  hotkey: string;
  variant: "again" | "hard" | "good" | "easy";
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
      { label: "Forgot", hint: card.again, hotkey: "1", variant: "again", rating: Rating.Again },
      { label: "Remembered", hint: card.good, hotkey: "2", variant: "good", rating: Rating.Good },
      { label: "Easy", hint: card.easy, hotkey: "3", variant: "easy", rating: Rating.Easy },
    ];
  }

  const isFsrs = card.algorithm === "fsrs";
  return [
    {
      label: isFsrs ? "Forgot" : "Again",
      hint: card.again,
      hotkey: "1",
      variant: "again",
      rating: Rating.Again,
    },
    { label: "Hard", hint: card.hard, hotkey: "2", variant: "hard", rating: Rating.Hard },
    {
      label: isFsrs ? "Remembered" : "Good",
      hint: card.good,
      hotkey: "3",
      variant: "good",
      rating: Rating.Good,
    },
    { label: "Easy", hint: card.easy, hotkey: "4", variant: "easy", rating: Rating.Easy },
  ];
}

function AnswerButton(props: {
  label: string;
  hint: string;
  hotkey: string;
  variant: "again" | "hard" | "good" | "easy";
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
