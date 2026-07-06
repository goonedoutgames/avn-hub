import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { Layout } from "@/components/Layout";
import { TaskProvider } from "@/context/TaskContext";
import { GameDetailPage } from "@/pages/GameDetailPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { MatchPage } from "@/pages/MatchPage";
import { SettingsPage } from "@/pages/SettingsPage";

function App() {
  return (
    <TaskProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<LibraryPage />} />
            <Route path="game/:id" element={<GameDetailPage />} />
            <Route path="match" element={<MatchPage />} />
            <Route path="settings" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </TaskProvider>
  );
}

export default App;
