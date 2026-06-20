import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { AppShell } from "@/app/AppShell";
import { RouteError } from "@/components/RouteError";
import { DeckBrowserScreen } from "@/routes/DeckBrowserScreen";
import { StudyScreen } from "@/routes/StudyScreen";
import { BrowseScreen } from "@/routes/BrowseScreen";
import { StatsScreen } from "@/routes/StatsScreen";
import { SettingsScreen } from "@/routes/SettingsScreen";

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
const statsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/stats",
  component: StatsScreen,
  errorComponent,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsScreen,
  errorComponent,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  studyRoute,
  browseRoute,
  statsRoute,
  settingsRoute,
]);

export const router = createRouter({ routeTree, defaultPreload: "intent" });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
