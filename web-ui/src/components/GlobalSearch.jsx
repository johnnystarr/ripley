import { useState, useEffect, useCallback, useRef } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faSearch,
  faTimes,
  faCompactDisc,
  faTv,
  faExclamationTriangle,
  faFileAlt,
  faSpinner,
} from '@fortawesome/free-solid-svg-icons';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';

export default function GlobalSearch({ isOpen, onClose }) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState({ shows: [], logs: [], issues: [] });
  const [loading, setLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef(null);
  const navigate = useNavigate();

  // Focus input when modal opens
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isOpen]);

  // Clear results when closing
  useEffect(() => {
    if (!isOpen) {
      setQuery('');
      setResults({ shows: [], logs: [], issues: [] });
      setSelectedIndex(0);
    }
  }, [isOpen]);

  // Search function with debouncing
  const performSearch = useCallback(async (searchQuery) => {
    if (!searchQuery.trim()) {
      setResults({ shows: [], logs: [], issues: [] });
      return;
    }

    setLoading(true);
    try {
      // Search in parallel
      const [shows, logs, issues] = await Promise.all([
        api.getShows().then(data => 
          data.filter(show => 
            show.name.toLowerCase().includes(searchQuery.toLowerCase())
          ).slice(0, 5)
        ),
        api.searchLogs({ query: searchQuery }).then(data => data.slice(0, 10)),
        api.getIssues().then(data => 
          data.filter(issue => 
            issue.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
            issue.description.toLowerCase().includes(searchQuery.toLowerCase())
          ).slice(0, 5)
        ),
      ]);

      setResults({ shows, logs, issues });
    } catch (err) {
      console.error('Search failed:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  // Debounced search
  useEffect(() => {
    const timer = setTimeout(() => {
      if (query) {
        performSearch(query);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [query, performSearch]);

  // Calculate total results
  const totalResults = results.shows.length + results.logs.length + results.issues.length;
  const allResults = [
    ...results.shows.map((item, idx) => ({ type: 'show', data: item, index: idx })),
    ...results.logs.map((item, idx) => ({ type: 'log', data: item, index: results.shows.length + idx })),
    ...results.issues.map((item, idx) => ({ type: 'issue', data: item, index: results.shows.length + results.logs.length + idx })),
  ];

  // Handle keyboard navigation
  const handleKeyDown = useCallback((e) => {
    if (e.key === 'Escape') {
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(prev => Math.min(prev + 1, totalResults - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(prev => Math.max(prev - 1, 0));
    } else if (e.key === 'Enter' && allResults[selectedIndex]) {
      e.preventDefault();
      handleSelectResult(allResults[selectedIndex]);
    }
  }, [totalResults, allResults, selectedIndex, onClose]);

  const handleSelectResult = useCallback((result) => {
    switch (result.type) {
      case 'show':
        navigate('/shows');
        break;
      case 'log':
        navigate('/logs');
        break;
      case 'issue':
        navigate('/issues');
        break;
    }
    onClose();
  }, [navigate, onClose]);

  if (!isOpen) return null;

  return (
    <>
      {/* Backdrop */}
      <div 
        className="fixed inset-0 bg-black bg-opacity-75 z-50"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] px-4">
        <div 
          className="bg-slate-800 rounded-lg shadow-2xl w-full max-w-2xl border border-slate-700"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Search Input */}
          <div className="flex items-center p-4 border-b border-slate-700">
            <FontAwesomeIcon icon={faSearch} className="text-slate-400 mr-3" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Search shows, logs, issues..."
              className="flex-1 bg-transparent text-slate-100 placeholder-slate-500 focus:outline-none text-lg"
            />
            {loading && (
              <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 animate-spin mr-3" />
            )}
            <button
              onClick={onClose}
              className="text-slate-400 hover:text-slate-300 p-2"
            >
              <FontAwesomeIcon icon={faTimes} />
            </button>
          </div>

          {/* Results */}
          <div className="max-h-[60vh] overflow-y-auto">
            {query && totalResults === 0 && !loading && (
              <div className="p-8 text-center text-slate-400">
                No results found for "{query}"
              </div>
            )}

            {!query && (
              <div className="p-8 text-center text-slate-400">
                <p className="mb-2">Quick search across all data</p>
                <p className="text-sm">
                  <kbd className="px-2 py-1 bg-slate-900 rounded text-xs">⌘K</kbd> or{' '}
                  <kbd className="px-2 py-1 bg-slate-900 rounded text-xs">Ctrl+K</kbd> to open
                </p>
              </div>
            )}

            {/* Shows */}
            {results.shows.length > 0 && (
              <div className="border-b border-slate-700">
                <div className="px-4 py-2 bg-slate-900/50 text-slate-400 text-sm font-semibold">
                  Shows
                </div>
                {results.shows.map((show, idx) => {
                  const resultIndex = idx;
                  return (
                    <button
                      key={show.id}
                      onClick={() => handleSelectResult({ type: 'show', data: show })}
                      className={`w-full px-4 py-3 flex items-center hover:bg-slate-700/50 transition-colors text-left ${
                        selectedIndex === resultIndex ? 'bg-slate-700/50' : ''
                      }`}
                    >
                      <FontAwesomeIcon icon={faTv} className="text-cyan-400 mr-3" />
                      <span className="text-slate-100">{show.name}</span>
                    </button>
                  );
                })}
              </div>
            )}

            {/* Logs */}
            {results.logs.length > 0 && (
              <div className="border-b border-slate-700">
                <div className="px-4 py-2 bg-slate-900/50 text-slate-400 text-sm font-semibold">
                  Logs
                </div>
                {results.logs.map((log, idx) => {
                  const resultIndex = results.shows.length + idx;
                  return (
                    <button
                      key={idx}
                      onClick={() => handleSelectResult({ type: 'log', data: log })}
                      className={`w-full px-4 py-3 flex items-start hover:bg-slate-700/50 transition-colors text-left ${
                        selectedIndex === resultIndex ? 'bg-slate-700/50' : ''
                      }`}
                    >
                      <FontAwesomeIcon icon={faFileAlt} className="text-blue-400 mr-3 mt-1" />
                      <div className="flex-1 min-w-0">
                        <p className="text-slate-100 truncate">{log.message}</p>
                        <p className="text-xs text-slate-400 mt-1">
                          {new Date(log.timestamp).toLocaleString()}
                        </p>
                      </div>
                    </button>
                  );
                })}
              </div>
            )}

            {/* Issues */}
            {results.issues.length > 0 && (
              <div>
                <div className="px-4 py-2 bg-slate-900/50 text-slate-400 text-sm font-semibold">
                  Issues
                </div>
                {results.issues.map((issue, idx) => {
                  const resultIndex = results.shows.length + results.logs.length + idx;
                  return (
                    <button
                      key={issue.id}
                      onClick={() => handleSelectResult({ type: 'issue', data: issue })}
                      className={`w-full px-4 py-3 flex items-start hover:bg-slate-700/50 transition-colors text-left ${
                        selectedIndex === resultIndex ? 'bg-slate-700/50' : ''
                      }`}
                    >
                      <FontAwesomeIcon icon={faExclamationTriangle} className="text-yellow-400 mr-3 mt-1" />
                      <div className="flex-1 min-w-0">
                        <p className="text-slate-100 font-semibold truncate">{issue.title}</p>
                        <p className="text-sm text-slate-400 truncate">{issue.description}</p>
                      </div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="px-4 py-3 border-t border-slate-700 flex items-center justify-between text-xs text-slate-400">
            <div className="flex items-center gap-4">
              <span>
                <kbd className="px-2 py-1 bg-slate-900 rounded">↑↓</kbd> Navigate
              </span>
              <span>
                <kbd className="px-2 py-1 bg-slate-900 rounded">Enter</kbd> Select
              </span>
              <span>
                <kbd className="px-2 py-1 bg-slate-900 rounded">Esc</kbd> Close
              </span>
            </div>
            {totalResults > 0 && (
              <span>{totalResults} result{totalResults !== 1 ? 's' : ''}</span>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
