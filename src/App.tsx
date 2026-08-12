import type { JSX, ReactNode } from "react";
import { BrowserRouter, NavLink, Route, Routes } from "react-router-dom";
import Folders from "./routes/Folders";
import Logo from "./components/Logo";
import "./App.css";

interface NavItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

// Verlauf and Über are cut from this build by explicit deadline decision, and
// Einstellungen has no screen yet either (it shares the not-yet-built Task 9
// with Über). Only Ordner has a real route, so it is the only entry here — a
// route pointing at a component that does not exist would be worse than a
// short sidebar.
const NAV_ITEMS: NavItem[] = [{ to: "/", label: "Ordner", icon: "📁", end: true }];

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
  return (
    <BrowserRouter>
      <div className="app-shell">
        <Sidebar />
        <main className="content">
          <Routes>
            <Route path="/" element={<Folders />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
