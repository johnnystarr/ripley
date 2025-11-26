import { useState, useEffect, useMemo, useCallback } from 'react';
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
import Dropdown from '../components/Dropdown';

// Custom debounce hook
function useDebounce(value, delay) {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedValue(value);
    }, delay);

    return () => {
      clearTimeout(handler);
    };
  }, [value, delay]);

  return debouncedValue;
}

export default function Logs() {
  const [logs, setLogs] = useState([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedLevel, setSelectedLevel] = useState('all');
  const [selectedDrive, setSelectedDrive] = useState('all');

  // Debounce search query
  const debouncedSearchQuery = useDebounce(searchQuery, 500);

  useEffect(() => {
    fetchLogs();
  }, []);

  const fetchLogs = useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.getLogs();
      setLogs(data);
    } catch (err) {
      console.error('Failed to fetch logs:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  const handleSearch = useCallback(async () => {
    try {
      setLoading(true);
      const params = {};
      if (debouncedSearchQuery) params.query = debouncedSearchQuery;
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
  }, [debouncedSearchQuery, selectedLevel, selectedDrive]);

  // Auto-search when debounced query or filters change
  useEffect(() => {
    if (debouncedSearchQuery || selectedLevel !== 'all' || selectedDrive !== 'all') {
      handleSearch();
    }
  }, [debouncedSearchQuery, selectedLevel, selectedDrive, handleSearch]);

  const handleClearFilters = () => {
    setSearchQuery('');
    setSelectedLevel('all');
    setSelectedDrive('all');
    fetchLogs();
  };

  // Memoized helper functions
  const getLogIcon = useCallback((level) => {
    switch (level) {
      case 'error': return faCircleXmark;
      case 'warning': return faTriangleExclamation;
      case 'success': return faCircleCheck;
      default: return faCircleInfo;
    }
  }, []);

  const getLogColor = useCallback((level) => {
    switch (level) {
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      case 'success': return 'text-green-400';
      default: return 'text-blue-400';
    }
  }, []);

  // Memoize unique drives for filter dropdown
  const uniqueDrives = useMemo(() => {
    const drives = new Set(logs.filter(log => log.drive).map(log => log.drive));
    return Array.from(drives).sort();
  }, [logs]);

  const getLogBgColor = (level) => {
    switch (level) {
      case 'error': return 'bg-red-500/10 border-red-500/30';
      case 'warning': return 'bg-yellow-500/10 border-yellow-500/30';
      case 'success': return 'bg-green-500/10 border-green-500/30';
      default: return 'bg-blue-500/10 border-blue-500/30';
    }
  };

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
          <Dropdown
            label="Level"
            value={selectedLevel}
            onChange={(value) => setSelectedLevel(value)}
            options={[
              { value: 'all', label: 'All Levels' },
              { value: 'info', label: 'Info' },
              { value: 'success', label: 'Success' },
              { value: 'warning', label: 'Warning' },
              { value: 'error', label: 'Error' },
            ]}
          />

          {/* Drive Filter */}
          <Dropdown
            label="Drive"
            value={selectedDrive}
            onChange={(value) => setSelectedDrive(value)}
            options={[
              { value: 'all', label: 'All Drives' },
              ...uniqueDrives.map(drive => ({
                value: drive,
                label: drive
              }))
            ]}
          />
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
