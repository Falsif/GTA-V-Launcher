import { useApp } from "./state";
import { DialogHost, Sidebar, TitleBar, Toasts } from "./components/ui";
import HomePage from "./pages/Home";
import ModsPage from "./pages/Mods";
import GameSettingsPage from "./pages/GameSettings";
import ProfilesPage from "./pages/Profiles";
import ToolsPage from "./pages/Tools";
import LauncherPage from "./pages/Launcher";

export default function App() {
  const { config, page, background } = useApp();
  const dim = (config?.launcher.background_dim ?? 70) / 100;

  return (
    <div className="app">
      {background && (
        <div className="app-bg" style={{ backgroundImage: `url(${background})` }}>
          <div className="app-bg-dim" style={{ opacity: dim }} />
        </div>
      )}
      <TitleBar />
      <div className="app-body">
        <Sidebar />
        <main className="content" key={page}>
          {!config ? (
            <div className="loading">
              <div className="spinner" />
            </div>
          ) : page === "home" ? (
            <HomePage />
          ) : page === "mods" ? (
            <ModsPage />
          ) : page === "settings" ? (
            <GameSettingsPage />
          ) : page === "profiles" ? (
            <ProfilesPage />
          ) : page === "tools" ? (
            <ToolsPage />
          ) : (
            <LauncherPage />
          )}
        </main>
      </div>
      <Toasts />
      <DialogHost />
    </div>
  );
}
