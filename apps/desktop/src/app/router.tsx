import { createRootRoute, createRoute, createRouter, redirect } from "@tanstack/react-router";
import { AppShell } from "@/app/AppShell";
import { RouteError } from "@/components/RouteError";
import { AddScreen } from "@/routes/AddScreen";
import { DeckBrowserScreen } from "@/routes/DeckBrowserScreen";
import { StudySessionScreen } from "@/routes/StudySessionScreen";
import { BrowseScreen } from "@/routes/BrowseScreen";
import { StatsScreen } from "@/routes/StatsScreen";
import { SettingsScreen } from "@/routes/SettingsScreen";
import { NotetypeScreen } from "@/routes/NotetypeScreen";

const rootRoute = createRootRoute({ component: AppShell });

const errorComponent = ({ error }: { error: unknown }) => <RouteError error={error} />;

export interface DecksSearch {
  create?: boolean;
}

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DeckBrowserScreen,
  errorComponent,
  validateSearch: (search: Record<string, unknown>): DecksSearch => ({
    create: search.create === true ? true : undefined,
  }),
});
const studySessionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/study/$deckId",
  component: StudySessionScreen,
  errorComponent,
  params: {
    parse: (raw: Record<string, string>) => {
      const deckId = Number(raw.deckId);
      if (!Number.isFinite(deckId)) throw new Error(`invalid deck id "${raw.deckId}"`);
      return { deckId };
    },
    stringify: (params: { deckId: number }) => ({ deckId: String(params.deckId) }),
  },
});
const studyRedirectRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/study",
  beforeLoad: () => {
    throw redirect({ to: "/" });
  },
});
export interface StatsSearch {
  deck?: number;
  range?: "7d" | "1m" | "3m" | "1y" | "all";
}

export interface BrowseSearch {
  q?: string;
  from?: "stats";
  backDeck?: number;
  backRange?: StatsSearch["range"];
}

const STATS_RANGES = ["7d", "1m", "3m", "1y", "all"] as const;

const browseRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/browse",
  component: BrowseScreen,
  errorComponent,
  validateSearch: (search: Record<string, unknown>): BrowseSearch => ({
    q: typeof search.q === "string" ? search.q : undefined,
    from: search.from === "stats" ? "stats" : undefined,
    backDeck: typeof search.backDeck === "number" ? search.backDeck : undefined,
    backRange: STATS_RANGES.includes(search.backRange as never)
      ? (search.backRange as StatsSearch["range"])
      : undefined,
  }),
});

const statsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/stats",
  component: StatsScreen,
  errorComponent,
  validateSearch: (search: Record<string, unknown>): StatsSearch => ({
    deck: typeof search.deck === "number" ? search.deck : undefined,
    range: STATS_RANGES.includes(search.range as never)
      ? (search.range as StatsSearch["range"])
      : undefined,
  }),
});
const addRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/add",
  component: AddScreen,
  errorComponent,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsScreen,
  errorComponent,
});
const notetypesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/notetypes",
  component: NotetypeScreen,
  errorComponent,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  studySessionRoute,
  studyRedirectRoute,
  browseRoute,
  addRoute,
  statsRoute,
  settingsRoute,
  notetypesRoute,
]);

export const router = createRouter({ routeTree, defaultPreload: "intent" });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
