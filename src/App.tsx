import type { JSX, ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { BrowserRouter, NavLink, Route, Routes } from "react-router-dom";
import Folders from "./routes/Folders";
import History from "./routes/History";
import About from "./routes/About";
import FolderForm from "./routes/FolderForm";
import Settings from "./routes/Settings";
import Logo from "./components/Logo";
import { api } from "./lib/api";
import { useJobNotifications } from "./lib/useJobNotifications";
import "./App.css";

interface NavItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

// Paths are German, like the rest of the UI strings and the existing
// /ordner/:id routes -- the implementation plan's English ones predate that.
const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Ordner", icon: "📁", end: true },
  { to: "/verlauf", label: "Verlauf", icon: "🧾" },
  { to: "/einstellungen", label: "Einstellungen", icon: "⚙️" },
  { to: "/ueber", label: "Über", icon: "ℹ️" },
];

function Sidebar(): JSX.Element {
  // Read from the running binary via a command rather than duplicated in the
  // frontend, so it can never drift from what the release workflow stamps
  // into Cargo.toml.
  const version = useQuery({ queryKey: ["appVersion"], queryFn: api.getAppVersion });

  return (
    <aside className="sidebar">
      <div className="brand">
        <Logo size={30} wordmark wordmarkColor="#f4f1ea" />
      </div>
      <nav className="nav" aria-label="Hauptnavigation">
        {NAV_ITEMS.map((item) => (
          <NavLink key={item.to} to={item.to} end={item.end} className="nav-link">
            <span className="nav-icon" aria-hidden="true">
              {item.icon}
            </span>
            <span className="nav-label">{item.label}</span>
          </NavLink>
        ))}
      </nav>
      <div className="sidebar-foot">
        <span>Ordner rein, Papier raus.</span>
        {version.data !== undefined && <span className="sidebar-version">v{version.data}</span>}
      </div>
    </aside>
  );
}

function App(): JSX.Element {
  useJobNotifications();

  return (
    <BrowserRouter>
      <div className="app-shell">
        <Sidebar />
        <main className="content">
          <Routes>
            <Route path="/" element={<Folders />} />
            <Route path="/ordner/neu" element={<FolderForm />} />
            <Route path="/ordner/:id" element={<FolderForm />} />
            <Route path="/verlauf" element={<History />} />
            <Route path="/einstellungen" element={<Settings />} />
            <Route path="/ueber" element={<About />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
