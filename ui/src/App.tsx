import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from './components/theme-provider';
import Layout from './components/Layout';
import Collections from './pages/Collections';
import Records from './pages/Records';
import Health from './pages/Health';
import Settings from './pages/Settings';

function App() {
  return (
    <ThemeProvider defaultTheme="system" storageKey="oxidedb-ui-theme">
      <Router>
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<Navigate to="/collections" replace />} />
            <Route path="collections" element={<Collections />} />
            <Route path="collections/:collection" element={<Records />} />
            <Route path="health" element={<Health />} />
            <Route path="settings" element={<Settings />} />
          </Route>
        </Routes>
      </Router>
    </ThemeProvider>
  );
}

export default App;
