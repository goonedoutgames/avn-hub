import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Layout } from "@/components/Layout";
import { AuthProvider, useAuth } from "@/context/AuthContext";
import { TaskProvider } from "@/context/TaskContext";
import { GameDetailPage } from "@/pages/GameDetailPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { AuthSetupBanner, LoginPage } from "@/pages/LoginPage";
import { MatchPage } from "@/pages/MatchPage";
import { SettingsPage } from "@/pages/SettingsPage";

function AppRoutes() {
  const { loading, needsLogin, needsSetup } = useAuth();

  if (loading) {
    return (
      <p className="p-8 text-[var(--color-muted-foreground)]">Loading…</p>
    );
  }

  if (needsLogin) {
    return <LoginPage />;
  }

  return (
    <>
      {needsSetup && <AuthSetupBanner />}
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<LibraryPage />} />
          <Route path="game/:id" element={<GameDetailPage />} />
          <Route path="match" element={<MatchPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </>
  );
}

function App() {
  return (
    <TaskProvider>
      <AuthProvider>
        <BrowserRouter>
          <AppRoutes />
        </BrowserRouter>
      </AuthProvider>
    </TaskProvider>
  );
}

export default App;
