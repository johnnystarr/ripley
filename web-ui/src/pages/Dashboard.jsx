import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faCompactDisc,
  faPlay,
  faStop,
  faSync,
  faSpinner,
} from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import { wsManager } from '../websocket';

export default function Dashboard() {
  const [status, setStatus] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  // Fetch initial status
  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 2000); // Poll every 2 seconds
    return () => clearInterval(interval);
  }, []);

  // Listen to WebSocket events
  useEffect(() => {
    const unsubscribers = [
      wsManager.on('StatusUpdate', ({ status }) => {
        setStatus(status);
      }),
      wsManager.on('RipProgress', ({ progress, message }) => {
        setStatus(prev => ({
          ...prev,
          progress,
          logs: [...(prev?.logs || []), `[${new Date().toLocaleTimeString()}] ${message}`].slice(-10),
        }));
      }),
      wsManager.on('Log', ({ message }) => {
        setStatus(prev => ({
          ...prev,
          logs: [...(prev?.logs || []), `[${new Date().toLocaleTimeString()}] ${message}`].slice(-10),
        }));
      }),
    ];

    return () => {
      unsubscribers.forEach(unsub => unsub());
    };
  }, []);

  const fetchStatus = async () => {
    try {
      const data = await api.getStatus();
      setStatus(data);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleStopRip = async () => {
    try {
      await api.stopRip();
      fetchStatus();
    } catch (err) {
      setError(err.message);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <h1 className="text-3xl font-bold text-slate-100">Dashboard</h1>

      {error && (
        <div className="bg-red-500/10 border border-red-500 text-red-400 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      {/* Current Status Card */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4">
          <FontAwesomeIcon icon={faCompactDisc} className="mr-2 text-cyan-400" />
          Current Status
        </h2>

        <div className="space-y-4">
          <div className="flex items-center">
            <span className="text-slate-400 w-32">Status:</span>
            <span className={`px-3 py-1 rounded-full text-sm font-medium ${
              status?.is_ripping
                ? 'bg-green-500/20 text-green-400'
                : 'bg-slate-700 text-slate-300'
            }`}>
              {status?.is_ripping ? (
                <>
                  <FontAwesomeIcon icon={faSpinner} className="mr-2 animate-spin" />
                  Ripping
                </>
              ) : (
                'Idle'
              )}
            </span>
          </div>

          {status?.current_disc && (
            <div className="flex items-center">
              <span className="text-slate-400 w-32">Disc:</span>
              <span className="text-slate-100">{status.current_disc}</span>
            </div>
          )}

          {status?.current_title && (
            <div className="flex items-center">
              <span className="text-slate-400 w-32">Current Title:</span>
              <span className="text-slate-100">{status.current_title}</span>
            </div>
          )}

          {status?.is_ripping && (
            <div>
              <div className="flex items-center justify-between mb-2">
                <span className="text-slate-400">Progress:</span>
                <span className="text-slate-100">{Math.round(status.progress * 100)}%</span>
              </div>
              <div className="w-full bg-slate-700 rounded-full h-2">
                <div
                  className="bg-cyan-500 h-2 rounded-full transition-all duration-300"
                  style={{ width: `${status.progress * 100}%` }}
                />
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Quick Actions */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4">Quick Actions</h2>
        
        <div className="flex space-x-4">
          <button
            onClick={() => window.location.href = '/drives'}
            disabled={status?.is_ripping}
            className="flex items-center px-4 py-2 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors"
          >
            <FontAwesomeIcon icon={faPlay} className="mr-2" />
            Start Rip
          </button>

          <button
            onClick={handleStopRip}
            disabled={!status?.is_ripping}
            className="flex items-center px-4 py-2 bg-red-500 hover:bg-red-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors"
          >
            <FontAwesomeIcon icon={faStop} className="mr-2" />
            Stop Rip
          </button>

          <button
            onClick={fetchStatus}
            className="flex items-center px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
          >
            <FontAwesomeIcon icon={faSync} className="mr-2" />
            Refresh
          </button>
        </div>
      </div>

      {/* Recent Logs */}
      {status?.logs && status.logs.length > 0 && (
        <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
          <h2 className="text-xl font-semibold text-slate-100 mb-4">Recent Logs</h2>
          <div className="bg-slate-900 rounded p-4 font-mono text-sm space-y-1 max-h-64 overflow-y-auto">
            {status.logs.map((log, index) => (
              <div key={index} className="text-slate-300">
                {log}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
