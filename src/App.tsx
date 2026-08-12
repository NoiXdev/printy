import type { JSX, ReactNode } from "react";
import { BrowserRouter, NavLink, Route, Routes } from "react-router-dom";
import Folders from "./routes/Folders";
import Settings from "./routes/Settings";
import Logo from "./components/Logo";
import { useJobNotifications } from "./lib/useJobNotifications";
import "./App.css";

interface NavItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

// Verlauf and Über are cut from this build by explicit deadline decision.
// Einstellungen now has a real screen (Task 9), so it joins Ordner here — a
// route pointing at a component that does not exist would still be worse than
// a short sidebar, which is why Verlauf and Über stay out.
const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Ordner", icon: "📁", end: true },
  { to: "/einstellungen", label: "Einstellungen", icon: "⚙️" },
];

function Sidebar(): JSX.Element {
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
      <div className="sidebar-foot">Ordner rein, Papier raus.</div>
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
            <Route path="/einstellungen" element={<Settings />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
