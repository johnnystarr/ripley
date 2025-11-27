import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faCompactDisc,
  faHdd,
  faSpinner,
  faCircleCheck,
  faCircleXmark,
  faEject,
  faExclamationTriangle,
  faEdit,
  faSave,
  faTimes,
  faBan,
  faSync,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';
import { wsManager } from '../websocket';
import Dropdown from '../components/Dropdown';
import { LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, PieChart, Pie, Cell } from 'recharts';

export default function Dashboard() {
  const [drives, setDrives] = useState([]);
  const [logs, setLogs] = useState([]);
  const [issues, setIssues] = useState([]);
  const [loading, setLoading] = useState(true);
  const [lastTitle, setLastTitle] = useState('');
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [shows, setShows] = useState([]);
  const [selectedShowId, setSelectedShowId] = useState(null);
  const [statistics, setStatistics] = useState(null);
  const [logLevelFilter, setLogLevelFilter] = useState('all');
  const [ripStartTimes, setRipStartTimes] = useState({}); // Track start times by drive
  const [elapsedTimes, setElapsedTimes] = useState({}); // Track elapsed time by drive
  const [failedRips, setFailedRips] = useState([]);
  const [showFailedRips, setShowFailedRips] = useState(false);
  const [ripHistory, setRipHistory] = useState([]);
  const logsEndRef = useRef(null);

  // Fetch drives and logs on mount
  useEffect(() => {
    fetchDrives();
    fetchLogs();
    fetchActiveIssues();
    fetchLastTitle();
    fetchShows();
    fetchStatistics();
    fetchFailedRips();
    fetchRipHistory();
    
    // Poll for drive changes every 3 seconds
    const driveInterval = setInterval(fetchDrives, 3000);
    
    return () => clearInterval(driveInterval);
  }, []);

  // Auto-scroll logs to bottom when new entries arrive
  useEffect(() => {
    if (logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs]);

  // Update elapsed times every second for active rips
  useEffect(() => {
    const interval = setInterval(() => {
      setElapsedTimes(prev => {
        const now = Date.now();
        const updated = {};
        let hasChanges = false;
        
        for (const [drive, startTime] of Object.entries(ripStartTimes)) {
          const elapsed = Math.floor((now - startTime) / 1000);
          if (prev[drive] !== elapsed) {
            hasChanges = true;
          }
          updated[drive] = elapsed;
        }
        
        return hasChanges ? updated : prev;
      });
    }, 1000);
    
    return () => clearInterval(interval);
  }, [ripStartTimes]);

  // Listen to WebSocket events for real-time updates
  useEffect(() => {
    const unsubscribers = [
      wsManager.on('Log', ({ level, message, drive }) => {
        const timestamp = new Date().toLocaleTimeString();
        const logEntry = { level, message, drive, timestamp };
        setLogs(prev => [logEntry, ...prev].slice(0, 100));
      }),
      wsManager.on('RipStarted', ({ disc, drive }) => {
        // Update drive with disc title when rip starts
        setDrives(prev => prev.map(d => 
          d.device === drive ? { ...d, disc_title: disc, progress: 0 } : d
        ));
        toast.success(`Started ripping: ${disc}`);
      }),
      wsManager.on('DriveDetected', ({ drive }) => {
        setDrives(prev => {
          const exists = prev.some(d => d.device === drive.device);
          if (!exists) {
            toast.success(`Drive detected: ${drive.device}`);
          }
          return exists ? prev : [...prev, drive];
        });
      }),
      wsManager.on('DriveRemoved', ({ device }) => {
        setDrives(prev => prev.filter(d => d.device !== device));
        toast(`Drive removed: ${device}`, { icon: '💿' });
      }),
      wsManager.on('DriveEjected', ({ device }) => {
        const timestamp = new Date().toLocaleTimeString();
        setLogs(prev => [{
          level: 'success',
          message: `Drive ${device} ejected`,
          drive: device,
          timestamp
        }, ...prev].slice(0, 100));
        toast.success(`Drive ejected: ${device}`);
      }),
      wsManager.on('IssueCreated', ({ issue }) => {
        setIssues(prev => [issue, ...prev]);
        toast.error(`Issue: ${issue.title}`);
      }),
      wsManager.on('RipProgress', ({ progress, message, drive }) => {
        setDrives(prev => prev.map(d => 
          d.device === drive ? { ...d, progress, status: message } : d
        ));
        
        // Start tracking time when rip begins (first progress update)
        setRipStartTimes(prev => {
          if (!prev[drive] && progress > 0) {
            return { ...prev, [drive]: Date.now() };
          }
          return prev;
        });
      }),
      wsManager.on('RipCompleted', ({ disc, drive }) => {
        toast.success(`Rip completed: ${disc}`);
        // Clear disc title and rip state
        setDrives(prev => prev.map(d => 
          d.device === drive ? { ...d, disc_title: null, progress: 0, status: null } : d
        ));
        // Clear start time when completed
        setRipStartTimes(prev => {
          const updated = { ...prev };
          delete updated[drive];
          return updated;
        });
        setElapsedTimes(prev => {
          const updated = { ...prev };
          delete updated[drive];
          return updated;
        });
        // Refresh statistics
        fetchStatistics();
      }),
      wsManager.on('RipError', ({ error, drive }) => {
        toast.error(`Rip error: ${error}`);
        // Clear disc title and rip state on error
        if (drive) {
          setDrives(prev => prev.map(d => 
            d.device === drive ? { ...d, disc_title: null, progress: 0, status: null } : d
          ));
          setRipStartTimes(prev => {
            const updated = { ...prev };
            delete updated[drive];
            return updated;
          });
          setElapsedTimes(prev => {
            const updated = { ...prev };
            delete updated[drive];
            return updated;
          });
        }
      }),
    ];

    return () => {
      unsubscribers.forEach(unsub => unsub());
    };
  }, []);

  const fetchDrives = async () => {
    try {
      const data = await api.detectDrives();
      setDrives(data);
    } catch (err) {
      console.error('Failed to fetch drives:', err);
    } finally {
      setLoading(false);
    }
  };

  const fetchLogs = useCallback(async () => {
    try {
      const data = await api.getLogs();
      setLogs(data.map(log => ({
        ...log,
        timestamp: new Date(log.timestamp).toLocaleTimeString()
      })));
    } catch (err) {
      console.error('Failed to fetch logs:', err);
    }
  }, []);

  const fetchActiveIssues = useCallback(async () => {
    try {
      const data = await api.getActiveIssues();
      setIssues(data);
    } catch (err) {
      console.error('Failed to fetch issues:', err);
    }
  }, []);

  const fetchLastTitle = useCallback(async () => {
    try {
      const data = await api.getLastTitle();
      if (data.title) {
        setLastTitle(data.title);
      }
    } catch (err) {
      console.error('Failed to fetch last title:', err);
    }
  }, []);

  const fetchShows = useCallback(async () => {
    try {
      const data = await api.getShows();
      setShows(data);
      // After fetching shows, get the last selected show ID
      try {
        const lastShowData = await api.getLastShowId();
        if (lastShowData.show_id) {
          setSelectedShowId(lastShowData.show_id);
        }
      } catch (err) {
        console.error('Failed to fetch last show ID:', err);
      }
    } catch (err) {
      console.error('Failed to fetch shows:', err);
    }
  }, []);

  const fetchStatistics = useCallback(async () => {
    try {
      const data = await api.getStatistics();
      setStatistics(data);
    } catch (err) {
      console.error('Failed to fetch statistics:', err);
    }
  }, []);

  const fetchFailedRips = useCallback(async () => {
    try {
      const data = await api.getRipHistory(10);
      const failed = data.filter(rip => rip.status === 'failed');
      setFailedRips(failed.slice(0, 5)); // Show last 5 failures
    } catch (err) {
      console.error('Failed to fetch rip history:', err);
    }
  }, []);

  const fetchRipHistory = useCallback(async () => {
    try {
      const data = await api.getRipHistory(30); // Get last 30 for charts
      setRipHistory(data);
    } catch (err) {
      console.error('Failed to fetch rip history:', err);
    }
  }, []);

  const handleShowSelect = useCallback(async (showId) => {
    try {
      await api.selectShow(showId);
      setSelectedShowId(showId);
      const show = shows.find(s => s.id === showId);
      if (show) {
        setLastTitle(show.name);
      }
      toast.success('Show selected - will be used for all new rips');
    } catch (err) {
      toast.error('Failed to select show: ' + err.message);
    }
  }, [shows]);

  const handleSaveTitle = useCallback(async () => {
    try {
      await api.setLastTitle(lastTitle);
      setIsEditingTitle(false);
      setSelectedShowId(null); // Clear show selection when manually setting title
      toast.success('Title saved - will be used for all new rips');
    } catch (err) {
      toast.error('Failed to save title: ' + err.message);
    }
  }, [lastTitle]);

  const handleResolveIssue = useCallback(async (issueId) => {
    try {
      await api.resolveIssue(issueId);
      setIssues(prev => prev.filter(i => i.id !== issueId));
      toast.success('Issue resolved');
    } catch (err) {
      toast.error('Failed to resolve issue: ' + err.message);
    }
  }, []);

  const handleEjectDrive = useCallback(async (device) => {
    try {
      await api.ejectDrive(device);
      toast.success(`Drive ${device} ejected`);
      // Refresh drives list
      fetchDrives();
    } catch (err) {
      toast.error('Failed to eject drive: ' + err.message);
    }
  }, []);

  const handleStopRip = useCallback(async () => {
    if (!window.confirm('Stop the current rip operation?')) {
      return;
    }
    try {
      await api.stopRip();
      toast.success('Rip operation stopped');
    } catch (err) {
      toast.error('Failed to stop rip: ' + err.message);
    }
  }, []);

  const handleRetryRip = useCallback(async (ripHistory) => {
    try {
      const params = {
        title: ripHistory.title,
        output_path: ripHistory.output_path,
      };
      await api.startRip(params);
      toast.success(`Retrying rip: ${ripHistory.title || 'Unknown'}`);
      setShowFailedRips(false);
    } catch (err) {
      toast.error('Failed to start rip: ' + err.message);
    }
  }, []);

  // Memoized helper functions
  const getLogColor = useCallback((level) => {
    switch (level) {
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      case 'success': return 'text-green-400';
      case 'info': return 'text-cyan-400';
      default: return 'text-slate-300';
    }
  }, []);

  const getIssueIcon = useCallback(() => {
    return faExclamationTriangle;
  }, []);

  const formatBytes = useCallback((bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }, []);

  const formatElapsedTime = useCallback((seconds) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hours > 0) {
      return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${secs}s`;
    } else {
      return `${secs}s`;
    }
  }, []);

  // Filter logs based on selected level
  const filteredLogs = useMemo(() => {
    if (logLevelFilter === 'all') {
      return logs;
    }
    return logs.filter(log => log.level === logLevelFilter);
  }, [logs, logLevelFilter]);

  // Prepare chart data
  const chartData = useMemo(() => {
    if (!ripHistory.length) return { daily: [], statusPie: [] };

    // Group by date
    const dailyStats = {};
    ripHistory.forEach(rip => {
      const date = new Date(rip.timestamp).toLocaleDateString();
      if (!dailyStats[date]) {
        dailyStats[date] = { date, success: 0, failed: 0, cancelled: 0 };
      }
      dailyStats[date][rip.status]++;
    });

    const daily = Object.values(dailyStats).slice(-14); // Last 14 days

    // Status distribution
    const statusCounts = { success: 0, failed: 0, cancelled: 0 };
    ripHistory.forEach(rip => {
      statusCounts[rip.status]++;
    });

    const statusPie = [
      { name: 'Success', value: statusCounts.success, color: '#22d3ee' },
      { name: 'Failed', value: statusCounts.failed, color: '#ef4444' },
      { name: 'Cancelled', value: statusCounts.cancelled, color: '#94a3b8' },
    ].filter(item => item.value > 0);

    return { daily, statusPie };
  }, [ripHistory]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Ripley Logo */}
      <div className="flex justify-center mb-8">
        <img 
          src="/ripley-logo.png" 
          alt="Ripley Logo" 
          className="w-[30%] h-auto"
        />
      </div>

      <h1 className="text-2xl md:text-3xl font-bold text-slate-100">Dashboard</h1>

      {/* Statistics Cards */}
      {statistics && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-slate-400 text-sm">Total Rips</p>
                <p className="text-2xl md:text-3xl font-bold text-slate-100 mt-1">{statistics.total_rips}</p>
              </div>
              <div className="bg-cyan-500/10 p-3 rounded-lg">
                <FontAwesomeIcon icon={faCompactDisc} className="text-cyan-400 text-2xl" />
              </div>
            </div>
          </div>

          <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-slate-400 text-sm">Success Rate</p>
                <p className="text-2xl md:text-3xl font-bold text-green-400 mt-1">
                  {statistics.success_rate.toFixed(1)}%
                </p>
              </div>
              <div className="bg-green-500/10 p-3 rounded-lg">
                <FontAwesomeIcon icon={faCircleCheck} className="text-green-400 text-2xl" />
              </div>
            </div>
          </div>

          <div 
            className="bg-slate-800 rounded-lg p-5 border border-slate-700 cursor-pointer hover:border-red-500/50 transition-colors"
            onClick={() => failedRips.length > 0 && setShowFailedRips(!showFailedRips)}
          >
            <div className="flex items-center justify-between">
              <div>
                <p className="text-slate-400 text-sm">Failed Rips</p>
                <p className="text-2xl md:text-3xl font-bold text-red-400 mt-1">{statistics.failed_rips}</p>
                {failedRips.length > 0 && (
                  <p className="text-xs text-slate-500 mt-1">Click to view recent failures</p>
                )}
              </div>
              <div className="bg-red-500/10 p-3 rounded-lg">
                <FontAwesomeIcon icon={faCircleXmark} className="text-red-400 text-2xl" />
              </div>
            </div>
          </div>

          <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-slate-400 text-sm">Storage Used</p>
                <p className="text-2xl md:text-3xl font-bold text-slate-100 mt-1">
                  {formatBytes(statistics.total_storage_bytes)}
                </p>
              </div>
              <div className="bg-slate-700 p-3 rounded-lg">
                <FontAwesomeIcon icon={faHdd} className="text-slate-400 text-2xl" />
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Charts */}
      {ripHistory.length > 0 && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {/* Daily Rips Chart */}
          {chartData.daily.length > 0 && (
            <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
              <h2 className="text-lg font-semibold text-slate-100 mb-4">Rip History (Last 14 Days)</h2>
              <ResponsiveContainer width="100%" height={250}>
                <BarChart data={chartData.daily}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                  <XAxis dataKey="date" stroke="#94a3b8" tick={{ fontSize: 12 }} />
                  <YAxis stroke="#94a3b8" tick={{ fontSize: 12 }} />
                  <Tooltip 
                    contentStyle={{ 
                      backgroundColor: '#1e293b', 
                      border: '1px solid #334155',
                      borderRadius: '8px',
                      color: '#f1f5f9'
                    }}
                  />
                  <Legend />
                  <Bar dataKey="success" fill="#22d3ee" name="Success" />
                  <Bar dataKey="failed" fill="#ef4444" name="Failed" />
                  <Bar dataKey="cancelled" fill="#94a3b8" name="Cancelled" />
                </BarChart>
              </ResponsiveContainer>
            </div>
          )}

          {/* Status Distribution */}
          {chartData.statusPie.length > 0 && (
            <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
              <h2 className="text-lg font-semibold text-slate-100 mb-4">Status Distribution</h2>
              <ResponsiveContainer width="100%" height={250}>
                <PieChart>
                  <Pie
                    data={chartData.statusPie}
                    cx="50%"
                    cy="50%"
                    labelLine={false}
                    label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`}
                    outerRadius={80}
                    fill="#8884d8"
                    dataKey="value"
                  >
                    {chartData.statusPie.map((entry, index) => (
                      <Cell key={`cell-${index}`} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip 
                    contentStyle={{ 
                      backgroundColor: '#1e293b', 
                      border: '1px solid #334155',
                      borderRadius: '8px',
                      color: '#f1f5f9'
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>
          )}
        </div>
      )}

      {/* Recent Failed Rips */}
      {showFailedRips && failedRips.length > 0 && (
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-red-400">Recent Failed Rips</h2>
            <button
              onClick={() => setShowFailedRips(false)}
              className="text-slate-400 hover:text-slate-300"
            >
              <FontAwesomeIcon icon={faTimes} />
            </button>
          </div>
          <div className="space-y-3">
            {failedRips.map((rip, idx) => (
              <div key={idx} className="bg-slate-900/50 rounded-lg p-4 border border-slate-700">
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <h3 className="font-semibold text-slate-100 mb-1">
                      {rip.title || 'Unknown Title'}
                    </h3>
                    <p className="text-sm text-red-400 mb-2">{rip.error_message}</p>
                    <div className="flex gap-4 text-xs text-slate-400">
                      <span>{new Date(rip.timestamp).toLocaleString()}</span>
                      {rip.drive && <span>Drive: {rip.drive}</span>}
                    </div>
                  </div>
                  <button
                    onClick={() => handleRetryRip(rip)}
                    className="ml-4 px-3 py-1.5 bg-cyan-500 hover:bg-cyan-600 text-white text-sm rounded transition-colors whitespace-nowrap"
                  >
                    <FontAwesomeIcon icon={faSync} className="mr-1" />
                    Retry
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Show Selection */}
      <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
        <div className="mb-3">
          <h2 className="text-lg font-semibold text-slate-100">Select Show</h2>
          <p className="text-slate-400 text-sm mt-1">
            Choose a show from your list - it will be used for all new rips until you change it
          </p>
        </div>
        
        {shows.length > 0 ? (
          <Dropdown
            label="Show"
            value={selectedShowId || ''}
            options={[
              { value: '', label: 'No show selected' },
              ...shows.map(show => ({ value: show.id, label: show.name }))
            ]}
            onChange={(value) => value && handleShowSelect(value)}
          />
        ) : (
          <div className="text-slate-400 text-center py-8">
            <p className="mb-3">No shows available</p>
            <a href="/shows" className="text-cyan-400 hover:text-cyan-300 underline">
              Add shows in the Shows tab
            </a>
          </div>
        )}
        
        {isEditingTitle && (
          <div className="mt-4 pt-4 border-t border-slate-700">
            <p className="text-slate-400 text-sm mb-2">Or enter a custom title:</p>
            <div className="flex gap-2">
              <input
                type="text"
                value={lastTitle}
                onChange={(e) => setLastTitle(e.target.value)}
                onKeyPress={(e) => e.key === 'Enter' && handleSaveTitle()}
                placeholder="e.g., Foster's Home for Imaginary Friends"
                className="flex-1 px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
                autoFocus
              />
              <button
                onClick={handleSaveTitle}
                className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
              >
                <FontAwesomeIcon icon={faSave} />
              </button>
              <button
                onClick={() => {
                  setIsEditingTitle(false);
                  fetchLastTitle();
                }}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
              >
                <FontAwesomeIcon icon={faTimes} />
              </button>
            </div>
          </div>
        )}
        
        {!isEditingTitle && shows.length > 0 && (
          <button
            onClick={() => setIsEditingTitle(true)}
            className="mt-3 text-cyan-400 hover:text-cyan-300 transition-colors text-sm"
          >
            <FontAwesomeIcon icon={faEdit} className="mr-2" />
            Use custom title instead
          </button>
        )}
      </div>

      {/* Active Issues */}
      {issues.length > 0 && (
        <div className="space-y-3">
          {issues.map((issue) => (
            <div key={issue.id} className="bg-red-500/10 border border-red-500 rounded-lg p-4">
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <div className="flex items-center mb-2">
                    <FontAwesomeIcon icon={getIssueIcon(issue.issue_type)} className="text-red-400 mr-2" />
                    <h3 className="text-red-400 font-semibold">{issue.title}</h3>
                    <span className="ml-3 px-2 py-1 text-xs rounded bg-red-500/20 text-red-300">
                      {issue.issue_type}
                    </span>
                  </div>
                  <p className="text-slate-300 text-sm mb-2">{issue.description}</p>
                  <p className="text-slate-500 text-xs">
                    {new Date(issue.timestamp).toLocaleString()}
                  </p>
                </div>
                <button
                  onClick={() => handleResolveIssue(issue.id)}
                  className="ml-4 px-3 py-1 bg-red-500 hover:bg-red-600 text-white text-sm rounded transition-colors"
                >
                  Resolve
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Detected Drives */}
      <div>
        <h2 className="text-xl font-semibold text-slate-100 mb-4">
          <FontAwesomeIcon icon={faHdd} className="mr-2 text-cyan-400" />
          Detected Drives ({drives.length})
        </h2>
        
        {drives.length === 0 ? (
          <div className="bg-slate-800 rounded-lg p-8 border border-slate-700 text-center">
            <FontAwesomeIcon icon={faCompactDisc} className="text-slate-600 text-4xl mb-3" />
            <p className="text-slate-400">No optical drives detected</p>
            <p className="text-slate-500 text-sm mt-2">Insert a disc to begin automatic ripping</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {drives.map((drive) => (
              <div key={drive.device} className="bg-slate-800 rounded-lg p-5 border border-slate-700">
                <div className="flex items-start justify-between mb-3">
                  <div className="flex-1">
                    <div className="flex items-center mb-1">
                      <FontAwesomeIcon icon={faCompactDisc} className="text-cyan-400 mr-2" />
                      <h3 className="text-slate-100 font-semibold">{drive.device}</h3>
                    </div>
                    <p className="text-slate-400 text-sm">{drive.name}</p>
                  </div>
                  {drive.has_disc ? (
                    <FontAwesomeIcon icon={faCircleCheck} className="text-green-400 text-lg" />
                  ) : (
                    <FontAwesomeIcon icon={faCircleXmark} className="text-slate-600 text-lg" />
                  )}
                </div>

                <div className="space-y-2 text-sm">
                  <div className="flex justify-between">
                    <span className="text-slate-400">Status:</span>
                    <span className={drive.has_disc ? 'text-green-400' : 'text-slate-500'}>
                      {drive.has_disc ? 'Disc Present' : 'No Disc'}
                    </span>
                  </div>
                  
                  {drive.disc_title && (
                    <div className="flex justify-between">
                      <span className="text-slate-400">Title:</span>
                      <span className="text-cyan-400 font-semibold">{drive.disc_title}</span>
                    </div>
                  )}
                  
                  {drive.disc_type && (
                    <div className="flex justify-between">
                      <span className="text-slate-400">Type:</span>
                      <span className="text-slate-300">{drive.disc_type}</span>
                    </div>
                  )}

                  {drive.progress !== undefined && drive.progress > 0 && (
                    <div className="mt-3">
                      <div className="flex justify-between text-xs mb-1">
                        <span className="text-slate-400">Progress:</span>
                        <span className="text-slate-300">{Math.round(drive.progress * 100)}%</span>
                      </div>
                      <div className="w-full bg-slate-700 rounded-full h-1.5">
                        <div
                          className="bg-cyan-500 h-1.5 rounded-full transition-all duration-300"
                          style={{ width: `${drive.progress * 100}%` }}
                        />
                      </div>
                      {elapsedTimes[drive.device] !== undefined && (
                        <div className="flex justify-between text-xs mt-2">
                          <span className="text-slate-400">Elapsed:</span>
                          <span className="text-cyan-400 font-mono">{formatElapsedTime(elapsedTimes[drive.device])}</span>
                        </div>
                      )}
                      {drive.status && (
                        <p className="text-xs text-slate-400 mt-1">{drive.status}</p>
                      )}
                      <button
                        onClick={handleStopRip}
                        className="mt-3 w-full bg-red-600 hover:bg-red-700 text-white py-1.5 px-3 rounded text-sm transition-colors duration-200 flex items-center justify-center"
                      >
                        <FontAwesomeIcon icon={faBan} className="mr-2" />
                        Cancel Rip
                      </button>
                    </div>
                  )}
                  
                  {drive.has_disc && !drive.progress && (
                    <button
                      onClick={() => handleEjectDrive(drive.device)}
                      className="mt-3 w-full bg-slate-700 hover:bg-slate-600 text-slate-200 py-1.5 px-3 rounded text-sm transition-colors duration-200 flex items-center justify-center"
                    >
                      <FontAwesomeIcon icon={faEject} className="mr-2" />
                      Eject
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Real-time Log Stream */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-semibold text-slate-100">Live Logs</h2>
          <div className="flex items-center gap-3">
            <Dropdown
              value={logLevelFilter}
              onChange={(value) => setLogLevelFilter(value)}
              options={[
                { value: 'all', label: 'All Levels' },
                { value: 'info', label: 'Info' },
                { value: 'success', label: 'Success' },
                { value: 'warning', label: 'Warning' },
                { value: 'error', label: 'Error' },
              ]}
            />
            {logs.length > 0 && (
              <button
                onClick={async () => {
                  if (window.confirm('Clear all logs? This cannot be undone.')) {
                    try {
                      await api.clearLogs();
                      setLogs([]);
                      toast.success('Logs cleared');
                    } catch (err) {
                      toast.error('Failed to clear logs: ' + err.message);
                    }
                  }
                }}
                className="px-3 py-1.5 bg-red-500 hover:bg-red-600 text-white text-sm rounded transition-colors"
              >
                Clear
              </button>
            )}
          </div>
        </div>
        <div className="bg-slate-900 rounded p-4 font-mono text-xs space-y-1 max-h-96 overflow-y-auto">
          {filteredLogs.length === 0 ? (
            <div className="text-slate-500 text-center py-8">
              {logs.length === 0 ? 'No logs yet' : `No ${logLevelFilter} logs`}
            </div>
          ) : (
            <>
              {filteredLogs.map((log, index) => (
                <div key={index} className={`${getLogColor(log.level)} flex items-start`}>
                  <span className="text-slate-500 mr-2 flex-shrink-0">[{log.timestamp}]</span>
                  {log.drive && <span className="text-slate-600 mr-2 flex-shrink-0">[{log.drive}]</span>}
                  <span className={getLogColor(log.level)}>{log.message}</span>
                </div>
              ))}
              <div ref={logsEndRef} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
