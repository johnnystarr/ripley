import { BrowserRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faHome,
  faCompactDisc,
  faCog,
  faFileAlt,
  faBars,
  faCircle,
  faTv,
  faExclamationTriangle,
  faSearch,
  faUser,
} from '@fortawesome/free-solid-svg-icons';
import { useState, useEffect } from 'react';
import { Toaster } from 'react-hot-toast';
import { wsManager } from './websocket';
import ErrorBoundary from './components/ErrorBoundary';
import { requestNotificationPermission, showRipNotification } from './utils/notifications';

// Import pages
import Dashboard from './pages/Dashboard';
import Configuration from './pages/Configuration';
import Logs from './pages/Logs';
import Shows from './pages/Shows';
import Issues from './pages/Issues';
import Preferences from './pages/Preferences';
import GlobalSearch from './components/GlobalSearch';
import Breadcrumbs from './components/Breadcrumbs';
import { api } from './api';

function App() {
  const [sidebarOpen, setSidebarOpen] = useState(window.innerWidth >= 1024); // Default closed on mobile
  const [wsConnected, setWsConnected] = useState(false);
  const [ripProgress, setRipProgress] = useState(null); // { disc: string, progress: number }
  const [searchOpen, setSearchOpen] = useState(false);
  const [soundEnabled, setSoundEnabled] = useState(true); // Default to enabled

  // Handle responsive sidebar
  useEffect(() => {
    const handleResize = () => {
      if (window.innerWidth >= 1024) {
        setSidebarOpen(true);
      }
    };
    
    const handleCloseSidebar = () => {
      setSidebarOpen(false);
    };
    
    window.addEventListener('resize', handleResize);
    window.addEventListener('closeSidebar', handleCloseSidebar);
    return () => {
      window.removeEventListener('resize', handleResize);
      window.removeEventListener('closeSidebar', handleCloseSidebar);
    };
  }, []);

  // Global search keyboard shortcut (Cmd/Ctrl+K)
  useEffect(() => {
    const handleKeyDown = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setSearchOpen(true);
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  useEffect(() => {
    // Request notification permission on mount
    requestNotificationPermission();

    // Load user preferences for sound notifications
    api.getPreferences()
      .then(prefs => setSoundEnabled(prefs.sound_notifications))
      .catch(err => console.warn('Failed to load sound preferences:', err));

    // Connect to WebSocket
    wsManager.connect();

    // Listen for connection status
    const unsubscribeConnection = wsManager.on('connection', ({ connected }) => {
      setWsConnected(connected);
    });

    // Listen for rip progress to update tab title
    const unsubscribeProgress = wsManager.on('RipProgress', (data) => {
      setRipProgress({
        disc: data.disc || 'Ripping',
        progress: Math.round((data.progress || 0) * 100),
      });
    });

    // Listen for rip completion events
    const unsubscribeRipComplete = wsManager.on('RipCompleted', (data) => {
      showRipNotification({
        title: data.disc_title || 'Unknown Disc',
        status: 'success',
        message: `Successfully ripped to ${data.output_path || 'output directory'}`,
        playSound: soundEnabled,
      });
      setRipProgress(null); // Clear progress
    });

    // Listen for rip error events
    const unsubscribeRipError = wsManager.on('RipError', (data) => {
      showRipNotification({
        title: data.disc_title || 'Unknown Disc',
        status: 'failed',
        message: data.error || 'Rip operation failed',
        playSound: soundEnabled,
      });
      setRipProgress(null); // Clear progress
    });

    return () => {
      unsubscribeConnection();
      unsubscribeProgress();
      unsubscribeRipComplete();
      unsubscribeRipError();
      wsManager.disconnect();
    };
  }, []);

  // Update document title with rip progress
  useEffect(() => {
    if (ripProgress) {
      document.title = `(${ripProgress.progress}%) ${ripProgress.disc} - Ripley`;
    } else {
      document.title = 'Ripley - DVD/Blu-ray Ripper';
    }
  }, [ripProgress]);

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
      <GlobalSearch isOpen={searchOpen} onClose={() => setSearchOpen(false)} />
      <div className="flex h-screen bg-slate-900">
        {/* Sidebar */}
        <Sidebar isOpen={sidebarOpen} wsConnected={wsConnected} />

        {/* Main content */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Header */}
          <Header 
            toggleSidebar={() => setSidebarOpen(!sidebarOpen)}
            wsConnected={wsConnected}
            onSearchOpen={() => setSearchOpen(true)}
          />

          {/* Page content */}
          <main className="flex-1 overflow-x-hidden overflow-y-auto bg-slate-900 p-6">
            <ErrorBoundary>
              <Breadcrumbs />
              <Routes>
                <Route path="/" element={<ErrorBoundary><Dashboard /></ErrorBoundary>} />
                <Route path="/shows" element={<ErrorBoundary><Shows /></ErrorBoundary>} />
                <Route path="/issues" element={<ErrorBoundary><Issues /></ErrorBoundary>} />
                <Route path="/configuration" element={<ErrorBoundary><Configuration /></ErrorBoundary>} />
                <Route path="/preferences" element={<ErrorBoundary><Preferences /></ErrorBoundary>} />
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
    { path: '/shows', icon: faTv, label: 'Shows' },
    { path: '/issues', icon: faExclamationTriangle, label: 'Issues' },
    { path: '/configuration', icon: faCog, label: 'Configuration' },
    { path: '/preferences', icon: faUser, label: 'Preferences' },
    { path: '/logs', icon: faFileAlt, label: 'Logs' },
  ];

  return (
    <>
      {/* Mobile overlay */}
      {isOpen && (
        <div 
          className="fixed inset-0 bg-black bg-opacity-50 z-40 lg:hidden"
          onClick={() => {
            if (window.innerWidth < 1024) {
              // Close sidebar when clicking overlay on mobile
              const event = new CustomEvent('closeSidebar');
              window.dispatchEvent(event);
            }
          }}
        />
      )}
      
      {/* Sidebar */}
      <div className={`
        bg-slate-800 border-r border-slate-700 transition-all duration-300
        ${isOpen ? 'w-64' : 'w-0'} overflow-hidden
        lg:relative fixed inset-y-0 left-0 z-50
      `}>
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
    </>
  );
}

function Header({ toggleSidebar, wsConnected, onSearchOpen }) {
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
          <button
            onClick={onSearchOpen}
            className="hidden sm:flex items-center px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-400 hover:text-slate-300 hover:border-slate-600 transition-colors"
          >
            <FontAwesomeIcon icon={faSearch} className="mr-2" />
            <span className="text-sm">Search</span>
            <kbd className="ml-2 px-2 py-0.5 bg-slate-800 rounded text-xs">⌘K</kbd>
          </button>
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
