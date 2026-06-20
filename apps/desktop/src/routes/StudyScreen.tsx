import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpen, Check, Layers } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { ipc, isTauri, Rating, type RatingValue } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { resolveCardMedia } from "@/lib/media";

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

  const cardQuery = useQuery({
    queryKey: queryKeys.queue(String(deckId)),
    queryFn: () => ipc.getNextCard(deckId),
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

  const hitSessionCap = sessionCap > 0 && answeredCount >= sessionCap;

  const card = cardQuery.data ?? null;

  // Keyboard: Space/Enter reveals; 1–4 rate once revealed.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!card || answerMut.isPending || hitSessionCap) return;
      if (!revealed && (e.key === " " || e.key === "Enter")) {
        e.preventDefault();
        setRevealed(true);
        return;
      }
      if (revealed && ["1", "2", "3", "4"].includes(e.key)) {
        e.preventDefault();
        answerMut.mutate({ cardId: card.card_id, rating: Number(e.key) as RatingValue });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [card, revealed, answerMut, hitSessionCap]);

  const questionHtml = card ? (tauri ? resolveCardMedia(card.question) : card.question) : "";
  const answerHtml = card ? (tauri ? resolveCardMedia(card.answer) : card.answer) : "";

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
              key={card.card_id + (revealed ? "a" : "q")}
              dangerouslySetInnerHTML={{ __html: revealed ? answerHtml : questionHtml }}
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
