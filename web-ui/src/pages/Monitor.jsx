import { useState, useEffect, useCallback, useRef } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faSpinner,
  faCircleCheck,
  faCircleXmark,
  faPause,
  faPlay,
  faCompactDisc,
  faHdd,
  faChevronDown,
  faChevronUp,
  faExclamationTriangle,
  faCircle,
  faFilter,
  faHistory,
  faClock,
} from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import { wsManager } from '../websocket';
import toast from 'react-hot-toast';

export default function Monitor() {
  const [operations, setOperations] = useState([]);
  const [historyOperations, setHistoryOperations] = useState([]);
  const [drives, setDrives] = useState([]);
  const [loading, setLoading] = useState(true);
  const [expandedOperations, setExpandedOperations] = useState(new Set());
  const [operationLogs, setOperationLogs] = useState({}); // operation_id -> logs array
  const [statusFilter, setStatusFilter] = useState('all'); // all, running, completed, failed
  const [viewMode, setViewMode] = useState('active'); // 'active' or 'history'
  const logEndRefs = useRef({});

  const fetchOperationHistory = useCallback(async () => {
    try {
      const limit = 50;
      const status = statusFilter !== 'all' ? statusFilter : undefined;
      const data = await api.getOperationHistory(limit, status);
      setHistoryOperations(data);
    } catch (err) {
      console.error('Failed to fetch operation history:', err);
    }
  }, [statusFilter]);

  // Fetch initial data
  useEffect(() => {
    fetchOperations();
    fetchDrives();
    if (viewMode === 'history') {
      fetchOperationHistory();
    }
    
    // Poll for updates every 2 seconds
    const interval = setInterval(() => {
      fetchOperations();
      fetchDrives();
      if (viewMode === 'history') {
        fetchOperationHistory();
      }
    }, 2000);

    return () => clearInterval(interval);
  }, [viewMode, fetchOperationHistory]);

  // Set up WebSocket listeners for real-time updates
  useEffect(() => {
    const unsubscribeOperationStarted = wsManager.on('OperationStarted', (data) => {
      if (data.operation) {
        setOperations(prev => {
          const updated = [...prev];
          const existingIndex = updated.findIndex(op => op.operation_id === data.operation.operation_id);
          if (existingIndex >= 0) {
            updated[existingIndex] = data.operation;
          } else {
            updated.push(data.operation);
          }
          return updated;
        });
      }
    });

    const unsubscribeOperationProgress = wsManager.on('OperationProgress', (data) => {
      setOperations(prev => prev.map(op => 
        op.operation_id === data.operation_id
          ? { ...op, progress: data.progress, message: data.message }
          : op
      ));
    });

    const unsubscribeOperationCompleted = wsManager.on('OperationCompleted', (data) => {
      setOperations(prev => prev.map(op => 
        op.operation_id === data.operation_id
          ? { ...op, status: 'completed', progress: 100.0, completed_at: new Date().toISOString() }
          : op
      ));
    });

    const unsubscribeOperationFailed = wsManager.on('OperationFailed', (data) => {
      setOperations(prev => prev.map(op => 
        op.operation_id === data.operation_id
          ? { ...op, status: 'failed', error: data.error, completed_at: new Date().toISOString() }
          : op
      ));
    });

    // Listen for log events and associate with operations
    const unsubscribeLog = wsManager.on('Log', (data) => {
      if (data.operation_id) {
        setOperationLogs(prev => {
          const logs = prev[data.operation_id] || [];
          return {
            ...prev,
            [data.operation_id]: [...logs, {
              timestamp: new Date().toISOString(),
              level: data.level,
              message: data.message,
              drive: data.drive,
            }],
          };
        });
      }
    });

    return () => {
      unsubscribeOperationStarted();
      unsubscribeOperationProgress();
      unsubscribeOperationCompleted();
      unsubscribeOperationFailed();
      unsubscribeLog();
    };
  }, []);

  // Auto-scroll logs when new entries are added
  useEffect(() => {
    Object.keys(operationLogs).forEach(operationId => {
      if (expandedOperations.has(operationId) && logEndRefs.current[operationId]) {
        logEndRefs.current[operationId].scrollIntoView({ behavior: 'smooth' });
      }
    });
  }, [operationLogs, expandedOperations]);

  const fetchOperations = useCallback(async () => {
    try {
      const data = await api.getMonitorOperations();
      setOperations(data);
    } catch (err) {
      console.error('Failed to fetch operations:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchDrives = useCallback(async () => {
    try {
      const data = await api.getMonitorDrives();
      setDrives(data);
    } catch (err) {
      console.error('Failed to fetch drives:', err);
    }
  }, []);


  const toggleOperationExpansion = useCallback((operationId) => {
    setExpandedOperations(prev => {
      const newSet = new Set(prev);
      if (newSet.has(operationId)) {
        newSet.delete(operationId);
      } else {
        newSet.add(operationId);
        // Initialize logs array if not exists
        if (!operationLogs[operationId]) {
          setOperationLogs(prev => ({ ...prev, [operationId]: [] }));
        }
      }
      return newSet;
    });
  }, [operationLogs]);

  const getStatusColor = (status) => {
    switch (status) {
      case 'running': return 'text-cyan-400 bg-cyan-500/10 border-cyan-500/30';
      case 'completed': return 'text-green-400 bg-green-500/10 border-green-500/30';
      case 'failed': return 'text-red-400 bg-red-500/10 border-red-500/30';
      case 'paused': return 'text-yellow-400 bg-yellow-500/10 border-yellow-500/30';
      case 'queued': return 'text-blue-400 bg-blue-500/10 border-blue-500/30';
      default: return 'text-slate-400 bg-slate-700/50 border-slate-600/30';
    }
  };

  const getStatusIcon = (status) => {
    switch (status) {
      case 'running': return faCircle;
      case 'completed': return faCircleCheck;
      case 'failed': return faCircleXmark;
      case 'paused': return faPause;
      default: return faCircle;
    }
  };

  const getOperationTypeLabel = (type) => {
    switch (type) {
      case 'rip': return 'Rip';
      case 'upscale': return 'Upscale';
      case 'rename': return 'Rename';
      case 'transfer': return 'Transfer';
      default: return 'Operation';
    }
  };

  const formatDuration = (startTime, endTime) => {
    if (!startTime) return '';
    const start = new Date(startTime);
    const end = endTime ? new Date(endTime) : new Date();
    const diffMs = end - start;
    const diffSecs = Math.floor(diffMs / 1000);
    const diffMins = Math.floor(diffSecs / 60);
    const diffHours = Math.floor(diffMins / 60);
    
    if (diffHours > 0) {
      return `${diffHours}h ${diffMins % 60}m`;
    } else if (diffMins > 0) {
      return `${diffMins}m ${diffSecs % 60}s`;
    }
    return `${diffSecs}s`;
  };

  const filteredOperations = viewMode === 'active'
    ? operations.filter(op => {
        if (statusFilter === 'all') return true;
        return op.status === statusFilter;
      })
    : historyOperations.filter(op => {
        if (statusFilter === 'all') return true;
        return op.status === statusFilter;
      });

  const activeOperations = operations.filter(op => op.status === 'running' || op.status === 'paused');

  if (loading && operations.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl md:text-3xl font-bold text-slate-100">Monitor</h1>
          <p className="text-slate-400 mt-2 text-sm sm:text-base">
            {viewMode === 'active' 
              ? 'Real-time monitoring of all active operations'
              : 'View completed and failed operations from history'}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 bg-slate-800 rounded-lg p-1 border border-slate-700">
            <button
              onClick={() => setViewMode('active')}
              className={`px-3 py-1.5 rounded text-sm font-medium transition-colors ${
                viewMode === 'active'
                  ? 'bg-cyan-500 text-white'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              <FontAwesomeIcon icon={faClock} className="mr-2" />
              Active
            </button>
            <button
              onClick={() => {
                setViewMode('history');
                fetchOperationHistory();
              }}
              className={`px-3 py-1.5 rounded text-sm font-medium transition-colors ${
                viewMode === 'history'
                  ? 'bg-cyan-500 text-white'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              <FontAwesomeIcon icon={faHistory} className="mr-2" />
              History
            </button>
          </div>
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="px-4 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500 transition-colors"
          >
            <option value="all">All Operations</option>
            <option value="running">Running</option>
            <option value="paused">Paused</option>
            <option value="completed">Completed</option>
            <option value="failed">Failed</option>
            <option value="agent">Agent Operations</option>
          </select>
        </div>
      </div>

      {/* Two-panel layout */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left panel - Operations */}
        <div className="lg:col-span-2 space-y-4">
          {filteredOperations.length === 0 ? (
            <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
              <FontAwesomeIcon icon={viewMode === 'history' ? faHistory : faCompactDisc} className="text-slate-600 text-5xl mb-4" />
              <h3 className="text-xl font-semibold text-slate-100 mb-2">
                {viewMode === 'history' ? 'No History' : 'No Operations'}
              </h3>
              <p className="text-slate-400">
                {viewMode === 'history' 
                  ? 'No completed or failed operations found'
                  : 'No operations match the current filter'}
              </p>
            </div>
          ) : (
            filteredOperations.map((operation) => {
              const isExpanded = expandedOperations.has(operation.operation_id);
              const logs = operationLogs[operation.operation_id] || [];
              
              return (
                <div
                  key={operation.operation_id}
                  className={`bg-slate-800 rounded-lg border transition-colors ${getStatusColor(operation.status)}`}
                >
                  <div className="p-4">
                    <div className="flex items-start justify-between mb-3">
                      <div className="flex-1">
                        <div className="flex items-center gap-3 mb-1">
                          <FontAwesomeIcon
                            icon={getStatusIcon(operation.status)}
                            className={`text-sm ${
                              operation.status === 'running' ? 'animate-pulse' : ''
                            }`}
                          />
                          <h3 className="font-semibold text-lg">
                            {getOperationTypeLabel(operation.operation_type)}
                            {operation.title && `: ${operation.title}`}
                          </h3>
                          <span className="px-2 py-1 text-xs rounded bg-slate-900/50">
                            {operation.status}
                          </span>
                        </div>
                        <p className="text-slate-300 text-sm">{operation.message}</p>
                        {operation.drive && (
                          <p className="text-slate-400 text-xs mt-1">
                            Drive: {operation.drive}
                          </p>
                        )}
                      </div>
                      <button
                        onClick={() => toggleOperationExpansion(operation.operation_id)}
                        className="text-slate-400 hover:text-slate-300 transition-colors"
                      >
                        <FontAwesomeIcon icon={isExpanded ? faChevronUp : faChevronDown} />
                      </button>
                    </div>

                    {/* Progress bar */}
                    <div className="mb-3">
                      <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
                        <span>{Math.round(operation.progress)}%</span>
                        <span>{formatDuration(operation.started_at, operation.completed_at)}</span>
                      </div>
                      <div className="w-full bg-slate-900/50 rounded-full h-2">
                        <div
                          className={`h-2 rounded-full transition-all duration-300 ${
                            operation.status === 'completed'
                              ? 'bg-green-500'
                              : operation.status === 'failed'
                              ? 'bg-red-500'
                              : 'bg-cyan-500'
                          }`}
                          style={{ width: `${Math.min(operation.progress, 100)}%` }}
                        />
                      </div>
                    </div>

                    {/* Expanded logs view */}
                    {isExpanded && (
                      <div className="mt-4 pt-4 border-t border-slate-700">
                        <h4 className="text-sm font-semibold text-slate-300 mb-2">Logs</h4>
                        <div className="bg-slate-900/50 rounded p-3 max-h-64 overflow-y-auto font-mono text-xs">
                          {logs.length === 0 ? (
                            <p className="text-slate-500">No logs available</p>
                          ) : (
                            logs.map((log, idx) => (
                              <div key={idx} className="mb-1">
                                <span className="text-slate-500">
                                  [{new Date(log.timestamp).toLocaleTimeString()}]
                                </span>
                                {log.drive && (
                                  <span className="text-slate-600 ml-1">[{log.drive}]</span>
                                )}
                                <span className={`ml-1 ${
                                  log.level === 'error' ? 'text-red-400' :
                                  log.level === 'warning' ? 'text-yellow-400' :
                                  log.level === 'success' ? 'text-green-400' :
                                  'text-cyan-400'
                                }`}>
                                  {log.message}
                                </span>
                              </div>
                            ))
                          )}
                          <div ref={el => logEndRefs.current[operation.operation_id] = el} />
                        </div>
                      </div>
                    )}

                    {operation.error && (
                      <div className="mt-3 p-3 bg-red-500/10 border border-red-500/30 rounded text-red-400 text-sm">
                        <strong>Error:</strong> {operation.error}
                      </div>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>

        {/* Right panel - Drive Information */}
        <div className="space-y-4">
          <div className="bg-slate-800 rounded-lg border border-slate-700 p-4">
            <h2 className="text-lg font-semibold text-slate-100 mb-4 flex items-center gap-2">
              <FontAwesomeIcon icon={faHdd} />
              Drives ({drives.length})
            </h2>
            {drives.length === 0 ? (
              <p className="text-slate-400 text-sm">No drives detected</p>
            ) : (
              <div className="space-y-3">
                {drives.map((drive) => {
                  const hasActiveOperation = activeOperations.some(
                    op => op.drive === drive.device
                  );
                  return (
                    <div
                      key={drive.device}
                      className={`p-3 rounded border ${
                        hasActiveOperation
                          ? 'bg-cyan-500/10 border-cyan-500/30'
                          : 'bg-slate-900/50 border-slate-700'
                      }`}
                    >
                      <div className="flex items-center justify-between mb-1">
                        <span className="font-medium text-slate-100 text-sm">
                          {drive.name || drive.device}
                        </span>
                        {hasActiveOperation && (
                          <span className="text-xs text-cyan-400 animate-pulse">●</span>
                        )}
                      </div>
                      <div className="text-xs text-slate-400 space-y-1">
                        <div>Device: {drive.device}</div>
                        <div>
                          Type: {drive.media_type === 'AudioCD' ? 'Audio CD' :
                                 drive.media_type === 'DVD' ? 'DVD' :
                                 drive.media_type === 'BluRay' ? 'Blu-ray' : 'None'}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Summary stats */}
          <div className="bg-slate-800 rounded-lg border border-slate-700 p-4">
            <h2 className="text-lg font-semibold text-slate-100 mb-4">Summary</h2>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-slate-400">Active Operations</span>
                <span className="text-cyan-400 font-medium">
                  {operations.filter(op => op.status === 'running').length}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Paused</span>
                <span className="text-yellow-400 font-medium">
                  {operations.filter(op => op.status === 'paused').length}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Completed</span>
                <span className="text-green-400 font-medium">
                  {operations.filter(op => op.status === 'completed').length}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-400">Failed</span>
                <span className="text-red-400 font-medium">
                  {operations.filter(op => op.status === 'failed').length}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

