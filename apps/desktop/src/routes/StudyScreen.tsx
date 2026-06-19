import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpen, Check, Layers } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { ipc, isTauri, Rating, type RatingValue } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";

export function StudyScreen() {
  const tauri = isTauri();
  const [deckId, setDeckId] = useState<number | null>(null);

  if (deckId === null) {
    return <DeckPicker enabled={tauri} onPick={setDeckId} />;
  }
  return <Session deckId={deckId} onExit={() => setDeckId(null)} />;
}

function DeckPicker({ enabled, onPick }: { enabled: boolean; onPick: (id: number) => void }) {
  const decks = useQuery({ queryKey: queryKeys.decks, queryFn: ipc.listDecks, enabled });

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
          <ul className="mx-auto flex max-w-md flex-col gap-2">
            {(decks.data ?? []).map((deck) => (
              <li key={deck.id}>
                <button
                  className="flex w-full items-center gap-3 rounded-lg border border-border px-4 py-3 text-left text-sm font-medium transition-colors hover:bg-accent"
                  onClick={() => onPick(deck.id)}
                >
                  <Layers className="size-4 text-muted-foreground" />
                  {deck.name}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function Session({ deckId, onExit }: { deckId: number; onExit: () => void }) {
  const queryClient = useQueryClient();
  const [revealed, setRevealed] = useState(false);

  const cardQuery = useQuery({
    queryKey: queryKeys.queue(String(deckId)),
    queryFn: () => ipc.getNextCard(deckId),
  });

  const answerMut = useMutation({
    mutationFn: ({ cardId, rating }: { cardId: number; rating: RatingValue }) =>
      ipc.answerCard(cardId, rating),
    onSuccess: (next) => {
      queryClient.setQueryData(queryKeys.queue(String(deckId)), next ?? null);
      setRevealed(false);
    },
  });

  const card = cardQuery.data ?? null;

  // Keyboard: Space/Enter reveals; 1–4 rate once revealed.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!card || answerMut.isPending) return;
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
  }, [card, revealed, answerMut]);

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        title="Studying"
        actions={
          <Button variant="ghost" size="sm" onClick={onExit}>
            Back to decks
          </Button>
        }
      />
      {card ? (
        <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col px-6 py-8">
          <div className="synapse-card flex-1 rounded-xl border border-border bg-card p-8 text-card-foreground">
            <div dangerouslySetInnerHTML={{ __html: revealed ? card.answer : card.question }} />
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
