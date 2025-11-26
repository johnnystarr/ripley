import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faSearch,
  faFilter,
  faSpinner,
  faCircleInfo,
  faTriangleExclamation,
  faCircleXmark,
  faCircleCheck,
} from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';

export default function Logs() {
  const [logs, setLogs] = useState([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedLevel, setSelectedLevel] = useState('all');
  const [selectedDrive, setSelectedDrive] = useState('all');

  useEffect(() => {
    fetchLogs();
  }, []);

  const fetchLogs = async () => {
    try {
      setLoading(true);
      const data = await api.getLogs();
      setLogs(data);
    } catch (err) {
      console.error('Failed to fetch logs:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = async () => {
    try {
      setLoading(true);
      const params = {};
      if (searchQuery) params.query = searchQuery;
      if (selectedLevel !== 'all') params.level = selectedLevel;
      if (selectedDrive !== 'all') params.drive = selectedDrive;

      const data = Object.keys(params).length > 0 
        ? await api.searchLogs(params)
        : await api.getLogs();
      setLogs(data);
    } catch (err) {
      console.error('Failed to search logs:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleClearFilters = () => {
    setSearchQuery('');
    setSelectedLevel('all');
    setSelectedDrive('all');
    fetchLogs();
  };

  const getLogIcon = (level) => {
    switch (level) {
      case 'error': return faCircleXmark;
      case 'warning': return faTriangleExclamation;
      case 'success': return faCircleCheck;
      default: return faCircleInfo;
    }
  };

  const getLogColor = (level) => {
    switch (level) {
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      case 'success': return 'text-green-400';
      default: return 'text-blue-400';
    }
  };

  const getLogBgColor = (level) => {
    switch (level) {
      case 'error': return 'bg-red-500/10 border-red-500/30';
      case 'warning': return 'bg-yellow-500/10 border-yellow-500/30';
      case 'success': return 'bg-green-500/10 border-green-500/30';
      default: return 'bg-blue-500/10 border-blue-500/30';
    }
  };

  // Get unique drives from logs
  const uniqueDrives = [...new Set(logs.map(log => log.drive).filter(Boolean))];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-slate-100">Log History</h1>
        <button
          onClick={fetchLogs}
          className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
        >
          Refresh
        </button>
      </div>

      {/* Search and Filters */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          {/* Search */}
          <div className="md:col-span-2">
            <label className="block text-slate-400 text-sm mb-2">Search</label>
            <div className="relative">
              <FontAwesomeIcon 
                icon={faSearch} 
                className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-500" 
              />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
                placeholder="Search logs..."
                className="w-full pl-10 pr-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
              />
            </div>
          </div>

          {/* Level Filter */}
          <div>
            <label className="block text-slate-400 text-sm mb-2">Level</label>
            <select
              value={selectedLevel}
              onChange={(e) => setSelectedLevel(e.target.value)}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="all">All Levels</option>
              <option value="info">Info</option>
              <option value="success">Success</option>
              <option value="warning">Warning</option>
              <option value="error">Error</option>
            </select>
          </div>

          {/* Drive Filter */}
          <div>
            <label className="block text-slate-400 text-sm mb-2">Drive</label>
            <select
              value={selectedDrive}
              onChange={(e) => setSelectedDrive(e.target.value)}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="all">All Drives</option>
              {uniqueDrives.map(drive => (
                <option key={drive} value={drive}>{drive}</option>
              ))}
            </select>
          </div>
        </div>

        <div className="flex gap-3 mt-4">
          <button
            onClick={handleSearch}
            className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors flex items-center"
          >
            <FontAwesomeIcon icon={faFilter} className="mr-2" />
            Apply Filters
          </button>
          <button
            onClick={handleClearFilters}
            className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
          >
            Clear
          </button>
        </div>
      </div>

      {/* Log Entries */}
      {loading ? (
        <div className="flex items-center justify-center py-12">
          <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
        </div>
      ) : logs.length === 0 ? (
        <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
          <p className="text-slate-400">No logs found</p>
        </div>
      ) : (
        <div className="space-y-3">
          {logs.map((log, index) => (
            <div
              key={index}
              className={`rounded-lg p-4 border ${getLogBgColor(log.level)}`}
            >
              <div className="flex items-start">
                <FontAwesomeIcon
                  icon={getLogIcon(log.level)}
                  className={`${getLogColor(log.level)} text-lg mt-1 mr-3 flex-shrink-0`}
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-3 mb-1">
                    <span className={`font-semibold text-sm uppercase ${getLogColor(log.level)}`}>
                      {log.level}
                    </span>
                    {log.drive && (
                      <span className="px-2 py-0.5 bg-slate-700 text-slate-300 text-xs rounded">
                        {log.drive}
                      </span>
                    )}
                    <span className="text-slate-500 text-xs">
                      {new Date(log.timestamp).toLocaleString()}
                    </span>
                  </div>
                  <p className="text-slate-300 text-sm">{log.message}</p>
                  {log.disc && (
                    <p className="text-slate-500 text-xs mt-1">Disc: {log.disc}</p>
                  )}
                  {log.title && (
                    <p className="text-slate-500 text-xs">Title: {log.title}</p>
                  )}
                  {log.context && (
                    <p className="text-slate-400 text-xs mt-2 font-mono">{log.context}</p>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
