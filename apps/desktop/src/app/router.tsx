import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { AppShell } from "@/app/AppShell";
import { RouteError } from "@/components/RouteError";
import { AddScreen } from "@/routes/AddScreen";
import { DeckBrowserScreen } from "@/routes/DeckBrowserScreen";
import { StudyScreen } from "@/routes/StudyScreen";
import { BrowseScreen } from "@/routes/BrowseScreen";
import { StatsScreen } from "@/routes/StatsScreen";
import { SettingsScreen } from "@/routes/SettingsScreen";
import { NotetypeScreen } from "@/routes/NotetypeScreen";

const rootRoute = createRootRoute({ component: AppShell });

const errorComponent = ({ error }: { error: unknown }) => <RouteError error={error} />;

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DeckBrowserScreen,
  errorComponent,
});
const studyRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/study",
  component: StudyScreen,
  errorComponent,
});
const browseRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/browse",
  component: BrowseScreen,
  errorComponent,
});
export interface StatsSearch {
  deck?: number;
  range?: "7d" | "1m" | "3m" | "1y" | "all";
}

const statsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/stats",
  component: StatsScreen,
  errorComponent,
  validateSearch: (search: Record<string, unknown>): StatsSearch => ({
    deck: typeof search.deck === "number" ? search.deck : undefined,
    range: (["7d", "1m", "3m", "1y", "all"] as const).includes(search.range as never)
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
  studyRoute,
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
