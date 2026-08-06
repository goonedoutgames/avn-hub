import { Navigate, Route, Routes } from "react-router-dom";
import { useAuth } from "@/context/AuthContext";
import { Layout } from "@/components/Layout";
import { LoginPage } from "@/pages/LoginPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { BrowsePage } from "@/pages/BrowsePage";
import { GameDetailPage } from "@/pages/GameDetailPage";
import { SettingsPage } from "@/pages/SettingsPage";

function Protected({ children }: { children: React.ReactNode }) {
  const { loading, configured, authenticated } = useAuth();
  if (loading) {
    return <div className="muted p-8 text-center">Loading…</div>;
  }
  if (configured && !authenticated) {
    return <Navigate to="/login" replace />;
  }
  return children;
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route
        element={
          <Protected>
            <Layout />
          </Protected>
        }
      >
        <Route path="/" element={<LibraryPage />} />
        <Route path="/browse" element={<BrowsePage />} />
        <Route path="/game/:id" element={<GameDetailPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
