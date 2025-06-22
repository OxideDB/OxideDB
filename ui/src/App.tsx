import {
  BrowserRouter as Router,
  Routes,
  Route,
  Navigate,
} from "react-router-dom";
import { ThemeProvider } from "./components/theme-provider";
import { AuthProvider } from "./contexts/AuthContext";
import ProtectedRoute from "./components/ProtectedRoute";
import { SidebarProvider } from "./components/ui/sidebar";
import { AppSidebar } from "./components/AppSidebar";
import { Toaster } from "./components/ui/toaster";
import Login from "./pages/Login";
import Dashboard from "./pages/Dashboard";
import Collections from "./pages/Collections";
import Records from "./pages/Records";
import EditRecord from "./pages/EditRecord";
import CreateCollection from "./pages/CreateCollection";
import EditCollection from "./pages/EditCollection";
import Health from "./pages/Health";
import Settings from "./pages/Settings";
import Permissions from "./pages/Permissions";
import Plugins from "./pages/Plugins";
import Logs from "./pages/Logs";

function App() {
  return (
    <ThemeProvider defaultTheme="system" storageKey="oxidedb-ui-theme">
      <AuthProvider>
        <Router basename="/admin">
          <Routes>
            {/* Public login route */}
            <Route path="/login" element={<Login />} />

            {/* Protected admin routes */}
            <Route
              path="/*"
              element={
                <ProtectedRoute>
                  <SidebarProvider defaultOpen={true}>
                    <AppSidebar />
                    <main className="flex-1 min-w-0">
                      <Routes>
                        <Route index element={<Dashboard />} />
                        <Route path="dashboard" element={<Dashboard />} />
                        <Route path="collections" element={<Collections />} />
                        <Route
                          path="collections/new"
                          element={<CreateCollection />}
                        />
                        <Route
                          path="collections/:collection"
                          element={<Records />}
                        />
                        <Route
                          path="collections/:collection/edit"
                          element={<EditCollection />}
                        />
                        <Route
                          path="collections/:collection/new"
                          element={<EditRecord />}
                        />
                        <Route
                          path="collections/:collection/edit/:recordId"
                          element={<EditRecord />}
                        />
                        <Route path="health" element={<Health />} />
                        <Route
                          path="permissions"
                          element={
                            <ProtectedRoute requireSuperuser={true}>
                              <Permissions />
                            </ProtectedRoute>
                          }
                        />
                        <Route path="settings" element={<Settings />} />
                        <Route
                          path="plugins"
                          element={
                            <ProtectedRoute requireSuperuser={true}>
                              <Plugins />
                            </ProtectedRoute>
                          }
                        />
                        <Route path="logs" element={<Logs />} />
                      </Routes>
                    </main>
                    <Toaster />
                  </SidebarProvider>
                </ProtectedRoute>
              }
            />

            {/* Catch all - redirect to login */}
            <Route path="*" element={<Navigate to="/login" replace />} />
          </Routes>
        </Router>
      </AuthProvider>
    </ThemeProvider>
  );
}

export default App;
