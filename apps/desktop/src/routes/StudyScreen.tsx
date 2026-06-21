import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpen, Check, Flag, Layers, MinusCircle, SkipForward, Volume2 } from "lucide-react";
import renderMathInElement from "katex/contrib/auto-render";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { ipc, isTauri, Rating, type RatingValue } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { mediaUrl, prepareCard } from "@/lib/renderCard";

const FLAG_COLORS: Record<number, string> = {
  0: "text-muted-foreground",
  1: "text-red-500",
  2: "text-orange-500",
  3: "text-green-500",
  4: "text-blue-500",
};

export function StudyScreen() {
  const tauri = isTauri();
  const [session, setSession] = useState<{ deckId: number; sessionCap: number } | null>(null);

  if (session === null) {
    return (
      <DeckPicker
        enabled={tauri}
        onPick={(deckId, sessionCap) => setSession({ deckId, sessionCap })}
      />
    );
  }
  return (
    <Session
      deckId={session.deckId}
      sessionCap={session.sessionCap}
      onExit={() => setSession(null)}
    />
  );
}

function CountBadge({ count, color }: { count: number; color: string }) {
  if (count === 0) return null;
  return (
    <span className={`rounded px-1.5 py-0.5 text-xs font-semibold tabular-nums ${color}`}>
      {count}
    </span>
  );
}

function DeckPicker({
  enabled,
  onPick,
}: {
  enabled: boolean;
  onPick: (deckId: number, sessionCap: number) => void;
}) {
  const decks = useQuery({ queryKey: queryKeys.decks, queryFn: ipc.listDecks, enabled });
  const [sessionCap, setSessionCap] = useState(0);

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Study" description="Pick a deck to review." />
      <div className="flex-1 overflow-auto p-8">
        {!enabled ? (
          <EmptyState
            icon={BookOpen}
            title="Run the desktop app"
            description="Study runs against the Rust core over Tauri. Launch with `pnpm dev`."
          />
        ) : (
          <div className="mx-auto flex max-w-md flex-col gap-4">
            <ul className="flex flex-col gap-2">
              {(decks.data ?? []).map((deck) => (
                <li key={deck.id}>
                  <button
                    className="flex w-full items-center gap-3 rounded-lg border border-border px-4 py-3 text-left text-sm font-medium transition-colors hover:bg-accent"
                    onClick={() => onPick(deck.id, sessionCap)}
                  >
                    <Layers className="size-4 shrink-0 text-muted-foreground" />
                    <span className="flex-1 truncate">{deck.name}</span>
                    <span className="flex items-center gap-1">
                      <CountBadge
                        count={deck.new_count}
                        color="bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
                      />
                      <CountBadge
                        count={deck.learning_count}
                        color="bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300"
                      />
                      <CountBadge
                        count={deck.review_count}
                        color="bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300"
                      />
                    </span>
                  </button>
                </li>
              ))}
            </ul>
            <div className="flex items-center gap-2 rounded-lg border border-border bg-secondary/40 px-4 py-2.5 text-sm">
              <span className="text-muted-foreground">Study at most</span>
              <input
                type="number"
                min={0}
                max={9999}
                value={sessionCap}
                onChange={(e) => setSessionCap(Math.max(0, Number(e.target.value)))}
                className="h-7 w-20 rounded-md border border-input bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              <span className="text-muted-foreground">cards this session (0 = no limit)</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function Session({
  deckId,
  sessionCap,
  onExit,
}: {
  deckId: number;
  sessionCap: number;
  onExit: () => void;
}) {
  const tauri = isTauri();
  const queryClient = useQueryClient();
  const [revealed, setRevealed] = useState(false);
  const [answeredCount, setAnsweredCount] = useState(0);
  const [flagMenuOpen, setFlagMenuOpen] = useState(false);
  const flagRef = useRef<HTMLDivElement>(null);

  const cardQuery = useQuery({
    queryKey: queryKeys.queue(String(deckId)),
    queryFn: () => ipc.getNextCard(deckId),
    // Poll ONLY while no card is showing, so a matured learning card surfaces
    // without re-entering the deck. Never refetch while a card is on screen —
    // that would swap it mid-study and look like an auto-skip.
    refetchInterval: (query) => (query.state.data == null ? 15000 : false),
    refetchIntervalInBackground: false,
    // Don't swap the on-screen card just because the window regained focus.
    refetchOnWindowFocus: false,
  });

  const answerMut = useMutation({
    mutationFn: ({ cardId, rating }: { cardId: number; rating: RatingValue }) =>
      ipc.answerCard(cardId, rating),
    onSuccess: (next) => {
      setAnsweredCount((c) => c + 1);
      queryClient.setQueryData(queryKeys.queue(String(deckId)), next ?? null);
      setRevealed(false);
    },
  });

  // Card action mutations — each advances to the next card on success.
  const advanceAfterAction = (next: typeof cardQuery.data | null) => {
    queryClient.setQueryData(queryKeys.queue(String(deckId)), next ?? null);
    setRevealed(false);
  };

  const suspendMut = useMutation({
    mutationFn: (cardId: number) => ipc.suspendCards([cardId]),
    onSuccess: () => void queryClient.refetchQueries({ queryKey: queryKeys.queue(String(deckId)) }).then(() => setRevealed(false)),
  });

  const buryMut = useMutation({
    mutationFn: (cardId: number) => ipc.buryCards([cardId]),
    onSuccess: () => void queryClient.refetchQueries({ queryKey: queryKeys.queue(String(deckId)) }).then(() => setRevealed(false)),
  });

  const flagMut = useMutation({
    mutationFn: ({ cardId, flag }: { cardId: number; flag: number }) =>
      ipc.setCardFlag([cardId], flag),
    onSuccess: () => setFlagMenuOpen(false),
  });

  const hitSessionCap = sessionCap > 0 && answeredCount >= sessionCap;
  const card = cardQuery.data ?? null;
  const actionBusy = suspendMut.isPending || buryMut.isPending || flagMut.isPending;

  // Prepared card HTML + extracted sound list.
  const prepared = card
    ? {
        q: prepareCard(card.question, tauri),
        a: prepareCard(card.answer, tauri),
      }
    : null;
  const currentHtml = prepared ? (revealed ? prepared.a.html : prepared.q.html) : "";
  const currentSounds = prepared ? (revealed ? prepared.a.sounds : prepared.q.sounds) : [];

  // Audio sequencer: play sounds in document order; autoplay on card change / reveal.
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

  // KaTeX: run auto-render on card DOM after each HTML swap.
  const cardRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!cardRef.current) return;
    renderMathInElement(cardRef.current, {
      delimiters: [
        { left: "\\(", right: "\\)", display: false },
        { left: "\\[", right: "\\]", display: true },
        { left: "$$", right: "$$", display: true },
        { left: "$", right: "$", display: false },
      ],
      throwOnError: false,
    });
  }, [currentHtml]);

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

  // Keyboard: Space/Enter reveals; 1–4 rate once revealed; s=suspend b=bury; r=replay.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!card || answerMut.isPending || actionBusy || hitSessionCap) return;
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
        answerMut.mutate({ cardId: card.card_id, rating: Number(e.key) as RatingValue });
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
  }, [card, revealed, answerMut, hitSessionCap, actionBusy, suspendMut, buryMut, replayAudio]);

  // Unused helper kept for type narrowing.
  void advanceAfterAction;

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        title="Studying"
        actions={
          <div className="flex items-center gap-3">
            {card && (
              <span className="flex items-center gap-1.5 tabular-nums">
                {card.new_count > 0 && (
                  <span className="rounded px-1.5 py-0.5 text-xs font-semibold bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300">
                    {card.new_count}
                  </span>
                )}
                {card.learning_count > 0 && (
                  <span className="rounded px-1.5 py-0.5 text-xs font-semibold bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
                    {card.learning_count}
                  </span>
                )}
                {card.review_count > 0 && (
                  <span className="rounded px-1.5 py-0.5 text-xs font-semibold bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300">
                    {card.review_count}
                  </span>
                )}
              </span>
            )}
            <Button variant="ghost" size="sm" onClick={onExit}>
              Back to decks
            </Button>
          </div>
        }
      />
      {hitSessionCap ? (
        <EmptyState
          icon={Check}
          title="Session complete"
          description={`You've studied ${answeredCount} cards this session. Come back later to keep going.`}
          action={
            <Button variant="outline" onClick={onExit}>
              Back to decks
            </Button>
          }
        />
      ) : card ? (
        <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col px-6 py-8">
          <div className="synapse-card flex-1 rounded-xl border border-border bg-card p-8 text-card-foreground overflow-auto">
            <div
              ref={cardRef}
              key={card.card_id + (revealed ? "a" : "q")}
              dangerouslySetInnerHTML={{ __html: currentHtml }}
            />
          </div>

          <div className="mt-6">
            {revealed ? (
              <div className="grid grid-cols-4 gap-2">
                <AnswerButton
                  label="Again"
                  hint={card.again}
                  hotkey="1"
                  variant="destructive"
                  onClick={() => answerMut.mutate({ cardId: card.card_id, rating: Rating.Again })}
                />
                <AnswerButton
                  label="Hard"
                  hint={card.hard}
                  hotkey="2"
                  variant="secondary"
                  onClick={() => answerMut.mutate({ cardId: card.card_id, rating: Rating.Hard })}
                />
                <AnswerButton
                  label="Good"
                  hint={card.good}
                  hotkey="3"
                  variant="default"
                  onClick={() => answerMut.mutate({ cardId: card.card_id, rating: Rating.Good })}
                />
                <AnswerButton
                  label="Easy"
                  hint={card.easy}
                  hotkey="4"
                  variant="outline"
                  onClick={() => answerMut.mutate({ cardId: card.card_id, rating: Rating.Easy })}
                />
              </div>
            ) : (
              <Button className="w-full" onClick={() => setRevealed(true)}>
                Show answer <span className="ml-2 text-xs opacity-70">Space</span>
              </Button>
            )}

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
                {flagMenuOpen && (
                  <div
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
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      ) : (
        <EmptyState
          icon={Check}
          title="All done"
          description="No more cards due in this deck right now. Nice work."
          action={
            <Button variant="outline" onClick={onExit}>
              Back to decks
            </Button>
          }
        />
      )}
    </div>
  );
}

function AnswerButton(props: {
  label: string;
  hint: string;
  hotkey: string;
  variant: "default" | "secondary" | "outline" | "destructive";
  onClick: () => void;
}) {
  return (
    <Button
      variant={props.variant}
      className="h-auto flex-col gap-0.5 py-2"
      onClick={props.onClick}
    >
      <span className="text-xs opacity-70">{props.hint}</span>
      <span>
        {props.label} <span className="opacity-60">{props.hotkey}</span>
      </span>
    </Button>
  );
}
