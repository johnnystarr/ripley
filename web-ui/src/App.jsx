import { BrowserRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faHome,
  faCompactDisc,
  faCog,
  faFileAlt,
  faBars,
  faCircle,
} from '@fortawesome/free-solid-svg-icons';
import { useState, useEffect } from 'react';
import { Toaster } from 'react-hot-toast';
import { wsManager } from './websocket';
import ErrorBoundary from './components/ErrorBoundary';

// Import pages
import Dashboard from './pages/Dashboard';
import Configuration from './pages/Configuration';
import Logs from './pages/Logs';

function App() {
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [wsConnected, setWsConnected] = useState(false);

  useEffect(() => {
    // Connect to WebSocket
    wsManager.connect();

    // Listen for connection status
    const unsubscribe = wsManager.on('connection', ({ connected }) => {
      setWsConnected(connected);
    });

    return () => {
      unsubscribe();
      wsManager.disconnect();
    };
  }, []);

  return (
    <Router>
      <Toaster
        position="top-right"
        toastOptions={{
          duration: 4000,
          style: {
            background: '#1e293b',
            color: '#f1f5f9',
            border: '1px solid #334155',
          },
          success: {
            iconTheme: {
              primary: '#22d3ee',
              secondary: '#1e293b',
            },
          },
          error: {
            iconTheme: {
              primary: '#ef4444',
              secondary: '#1e293b',
            },
          },
        }}
      />
      <div className="flex h-screen bg-slate-900">
        {/* Sidebar */}
        <Sidebar isOpen={sidebarOpen} wsConnected={wsConnected} />

        {/* Main content */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Header */}
          <Header 
            toggleSidebar={() => setSidebarOpen(!sidebarOpen)}
            wsConnected={wsConnected}
          />

          {/* Page content */}
          <main className="flex-1 overflow-x-hidden overflow-y-auto bg-slate-900 p-6">
            <ErrorBoundary>
              <Routes>
                <Route path="/" element={<ErrorBoundary><Dashboard /></ErrorBoundary>} />
                <Route path="/configuration" element={<ErrorBoundary><Configuration /></ErrorBoundary>} />
                <Route path="/logs" element={<ErrorBoundary><Logs /></ErrorBoundary>} />
              </Routes>
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </Router>
  );
}

function Sidebar({ isOpen, wsConnected }) {
  const location = useLocation();

  const navItems = [
    { path: '/', icon: faHome, label: 'Dashboard' },
    { path: '/configuration', icon: faCog, label: 'Configuration' },
    { path: '/logs', icon: faFileAlt, label: 'Logs' },
  ];

  return (
    <div className={`bg-slate-800 border-r border-slate-700 transition-all duration-300 ${
      isOpen ? 'w-64' : 'w-0'
    } overflow-hidden`}>
      <div className="p-6">
        <h1 className="text-2xl font-bold text-yellow-400 mb-8 flex items-center">
          <img src="/ripley-head.png" alt="Ripley" className="w-[35px] h-auto mr-2" />
          RIPLEY
        </h1>

        <nav className="space-y-2">
          {navItems.map((item) => (
            <Link
              key={item.path}
              to={item.path}
              className={`flex items-center px-4 py-3 rounded-lg transition-colors ${
                location.pathname === item.path
                  ? 'bg-cyan-500 text-white'
                  : 'text-slate-300 hover:bg-slate-700'
              }`}
            >
              <FontAwesomeIcon icon={item.icon} className="w-5 mr-3" />
              {item.label}
            </Link>
          ))}
        </nav>

        {/* Connection status */}
        <div className="mt-8 pt-4 border-t border-slate-700">
          <div className="flex items-center text-sm">
            <FontAwesomeIcon
              icon={faCircle}
              className={`w-2 h-2 mr-2 ${
                wsConnected ? 'text-green-400' : 'text-red-400'
              }`}
            />
            <span className="text-slate-400">
              {wsConnected ? 'Connected' : 'Disconnected'}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

function Header({ toggleSidebar, wsConnected }) {
  return (
    <header className="bg-slate-800 border-b border-slate-700 px-6 py-4">
      <div className="flex items-center justify-between">
        <button
          onClick={toggleSidebar}
          className="text-slate-300 hover:text-white p-2 rounded-lg hover:bg-slate-700 transition-colors"
        >
          <FontAwesomeIcon icon={faBars} className="w-5 h-5" />
        </button>

        <div className="flex items-center space-x-4">
          <div className="flex items-center">
            <div className={`w-2 h-2 rounded-full mr-2 ${
              wsConnected ? 'bg-green-400 animate-pulse' : 'bg-red-400'
            }`} />
            <span className="text-sm text-slate-400">
              {wsConnected ? 'Live' : 'Offline'}
            </span>
          </div>
        </div>
      </div>
    </header>
  );
}

export default App;
