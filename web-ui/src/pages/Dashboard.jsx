import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faCompactDisc,
  faHdd,
  faSpinner,
  faCircleCheck,
  faCircleXmark,
  faEject,
  faExclamationTriangle,
} from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import { wsManager } from '../websocket';

export default function Dashboard() {
  const [drives, setDrives] = useState([]);
  const [logs, setLogs] = useState([]);
  const [issues, setIssues] = useState([]);
  const [loading, setLoading] = useState(true);

  // Fetch drives and logs on mount
  useEffect(() => {
    fetchDrives();
    fetchLogs();
    fetchActiveIssues();
    
    // Poll for drive changes every 3 seconds
    const driveInterval = setInterval(fetchDrives, 3000);
    
    return () => clearInterval(driveInterval);
  }, []);

  // Listen to WebSocket events for real-time updates
  useEffect(() => {
    const unsubscribers = [
      wsManager.on('Log', ({ level, message, drive }) => {
        const timestamp = new Date().toLocaleTimeString();
        const logEntry = { level, message, drive, timestamp };
        setLogs(prev => [logEntry, ...prev].slice(0, 100));
      }),
      wsManager.on('DriveDetected', ({ drive }) => {
        setDrives(prev => {
          const exists = prev.some(d => d.device === drive.device);
          return exists ? prev : [...prev, drive];
        });
      }),
      wsManager.on('DriveRemoved', ({ device }) => {
        setDrives(prev => prev.filter(d => d.device !== device));
      }),
      wsManager.on('DriveEjected', ({ device }) => {
        const timestamp = new Date().toLocaleTimeString();
        setLogs(prev => [{
          level: 'success',
          message: `Drive ${device} ejected`,
          drive: device,
          timestamp
        }, ...prev].slice(0, 100));
      }),
      wsManager.on('IssueCreated', ({ issue }) => {
        setIssues(prev => [issue, ...prev]);
      }),
      wsManager.on('RipProgress', ({ progress, message, drive }) => {
        setDrives(prev => prev.map(d => 
          d.device === drive ? { ...d, progress, status: message } : d
        ));
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

  const fetchLogs = async () => {
    try {
      const data = await api.getLogs();
      setLogs(data.map(log => ({
        ...log,
        timestamp: new Date(log.timestamp).toLocaleTimeString()
      })));
    } catch (err) {
      console.error('Failed to fetch logs:', err);
    }
  };

  const fetchActiveIssues = async () => {
    try {
      const data = await api.getActiveIssues();
      setIssues(data);
    } catch (err) {
      console.error('Failed to fetch issues:', err);
    }
  };

  const handleResolveIssue = async (issueId) => {
    try {
      await api.resolveIssue(issueId);
      setIssues(prev => prev.filter(i => i.id !== issueId));
    } catch (err) {
      console.error('Failed to resolve issue:', err);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  const getLogColor = (level) => {
    switch (level) {
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      case 'success': return 'text-green-400';
      default: return 'text-slate-300';
    }
  };

  const getIssueIcon = (type) => {
    return faExclamationTriangle;
  };

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

      <h1 className="text-3xl font-bold text-slate-100">Dashboard</h1>

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
                      {drive.status && (
                        <p className="text-xs text-slate-400 mt-1">{drive.status}</p>
                      )}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Real-time Log Stream */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4">Live Logs</h2>
        <div className="bg-slate-900 rounded p-4 font-mono text-xs space-y-1 max-h-96 overflow-y-auto">
          {logs.length === 0 ? (
            <div className="text-slate-500 text-center py-8">No logs yet</div>
          ) : (
            logs.map((log, index) => (
              <div key={index} className={`${getLogColor(log.level)} flex items-start`}>
                <span className="text-slate-500 mr-2 flex-shrink-0">[{log.timestamp}]</span>
                {log.drive && <span className="text-slate-600 mr-2 flex-shrink-0">[{log.drive}]</span>}
                <span className={getLogColor(log.level)}>{log.message}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
