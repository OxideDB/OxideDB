import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { ThemeProvider } from './components/theme-provider';
import Layout from './components/Layout';
import Collections from './pages/Collections';
import Records from './pages/Records';
import EditRecord from './pages/EditRecord';
import CreateCollection from './pages/CreateCollection';
import EditCollection from './pages/EditCollection';
import Health from './pages/Health';
import Settings from './pages/Settings';

function App() {
  return (
    <ThemeProvider defaultTheme="system" storageKey="oxidedb-ui-theme">
      <Router basename="/admin">
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<Navigate to="/collections" replace />} />
            <Route path="collections" element={<Collections />} />
            <Route path="collections/new" element={<CreateCollection />} />
            <Route path="collections/:collection" element={<Records />} />
            <Route path="collections/:collection/edit" element={<EditCollection />} />
            <Route path="collections/:collection/new" element={<EditRecord />} />
            <Route path="collections/:collection/edit/:recordId" element={<EditRecord />} />
            <Route path="health" element={<Health />} />
            <Route path="settings" element={<Settings />} />
          </Route>
        </Routes>
      </Router>
    </ThemeProvider>
  );
}

export default App;
