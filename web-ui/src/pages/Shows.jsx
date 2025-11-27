import { useState, useEffect, useCallback, useMemo } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faPlus,
  faEdit,
  faTrash,
  faSave,
  faTimes,
  faCheck,
  faSpinner,
  faTv,
  faSearch,
  faCheckSquare,
  faSquare,
  faFileExport,
  faFileImport,
  faChevronLeft,
  faChevronRight,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';

// Helper function to format relative time
function formatRelativeTime(dateString) {
  if (!dateString) return 'Never used';
  
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now - date;
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);
  
  if (diffSecs < 60) return 'Just now';
  if (diffMins < 60) return `${diffMins} minute${diffMins !== 1 ? 's' : ''} ago`;
  if (diffHours < 24) return `${diffHours} hour${diffHours !== 1 ? 's' : ''} ago`;
  if (diffDays < 7) return `${diffDays} day${diffDays !== 1 ? 's' : ''} ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)} week${Math.floor(diffDays / 7) !== 1 ? 's' : ''} ago`;
  if (diffDays < 365) return `${Math.floor(diffDays / 30)} month${Math.floor(diffDays / 30) !== 1 ? 's' : ''} ago`;
  return `${Math.floor(diffDays / 365)} year${Math.floor(diffDays / 365) !== 1 ? 's' : ''} ago`;
}

export default function Shows() {
  const [shows, setShows] = useState([]);
  const [loading, setLoading] = useState(true);
  const [newShowName, setNewShowName] = useState('');
  const [isAdding, setIsAdding] = useState(false);
  const [editingId, setEditingId] = useState(null);
  const [editingName, setEditingName] = useState('');
  const [selectedShowId, setSelectedShowId] = useState(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [sortBy, setSortBy] = useState('name-asc'); // name-asc, name-desc, date-asc, date-desc, last-used-asc, last-used-desc
  const [selectedShows, setSelectedShows] = useState(new Set());
  const [bulkMode, setBulkMode] = useState(false);
  const [currentPage, setCurrentPage] = useState(1);
  const [itemsPerPage, setItemsPerPage] = useState(25);

  useEffect(() => {
    fetchShows();
    fetchLastShow();
  }, []);

  const fetchShows = useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.getShows();
      setShows(data);
    } catch (err) {
      console.error('Failed to fetch shows:', err);
      toast.error('Failed to load shows');
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchLastShow = useCallback(async () => {
    try {
      const data = await api.getLastTitle();
      if (data.title) {
        // Find the show with this name
        const show = shows.find(s => s.name === data.title);
        if (show && show.id) {
          setSelectedShowId(show.id);
        }
      }
    } catch (err) {
      console.error('Failed to fetch last show:', err);
    }
  }, [shows]);

  const handleAddShow = useCallback(async () => {
    if (!newShowName.trim()) {
      toast.error('Please enter a show name');
      return;
    }

    try {
      await api.createShow(newShowName.trim());
      toast.success('Show added');
      setNewShowName('');
      setIsAdding(false);
      fetchShows();
    } catch (err) {
      toast.error('Failed to add show: ' + err.message);
    }
  }, [newShowName, fetchShows]);

  const handleUpdateShow = useCallback(async (id) => {
    if (!editingName.trim()) {
      toast.error('Please enter a show name');
      return;
    }

    try {
      await api.updateShow(id, editingName.trim());
      toast.success('Show updated');
      setEditingId(null);
      setEditingName('');
      fetchShows();
    } catch (err) {
      toast.error('Failed to update show: ' + err.message);
    }
  }, [editingName, fetchShows]);

  const handleDeleteShow = useCallback(async (id, name) => {
    if (!confirm(`Delete "${name}"?`)) {
      return;
    }

    try {
      await api.deleteShow(id);
      toast.success('Show deleted');
      if (selectedShowId === id) {
        setSelectedShowId(null);
      }
      fetchShows();
    } catch (err) {
      toast.error('Failed to delete show: ' + err.message);
    }
  }, [selectedShowId, fetchShows]);

  const handleSelectShow = useCallback(async (id) => {
    try {
      await api.selectShow(id);
      setSelectedShowId(id);
      toast.success('Show selected');
    } catch (err) {
      toast.error('Failed to select show: ' + err.message);
    }
  }, []);

  const handleToggleSelect = useCallback((id) => {
    setSelectedShows(prev => {
      const newSet = new Set(prev);
      if (newSet.has(id)) {
        newSet.delete(id);
      } else {
        newSet.add(id);
      }
      return newSet;
    });
  }, []);

  // Reset to page 1 when filters change
  useEffect(() => {
    setCurrentPage(1);
  }, [searchQuery, sortBy, itemsPerPage]);

  const handleSelectAll = useCallback(() => {
    if (selectedShows.size === paginatedShows.length && 
        paginatedShows.every(s => selectedShows.has(s.id))) {
      setSelectedShows(new Set());
    } else {
      const newSelected = new Set(selectedShows);
      paginatedShows.forEach(s => newSelected.add(s.id));
      setSelectedShows(newSelected);
    }
  }, [selectedShows, paginatedShows]);

  const handleBulkDelete = useCallback(async () => {
    if (selectedShows.size === 0) return;
    
    if (!confirm(`Delete ${selectedShows.size} show(s)?`)) {
      return;
    }

    try {
      await Promise.all(
        Array.from(selectedShows).map(id => api.deleteShow(id))
      );
      toast.success(`Deleted ${selectedShows.size} show(s)`);
      setSelectedShows(new Set());
      setBulkMode(false);
      fetchShows();
    } catch (err) {
      toast.error('Failed to delete shows: ' + err.message);
    }
  }, [selectedShows, fetchShows]);

  const handleExportShows = useCallback(() => {
    const dataStr = JSON.stringify(shows, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `ripley-shows-${new Date().toISOString().split('T')[0]}.json`;
    link.click();
    URL.revokeObjectURL(url);
    toast.success('Shows exported');
  }, [shows]);

  const handleImportShows = useCallback((event) => {
    const file = event.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async (e) => {
      try {
        const imported = JSON.parse(e.target.result);
        if (!Array.isArray(imported)) {
          throw new Error('Invalid format: expected array');
        }

        let imported_count = 0;
        for (const show of imported) {
          if (show.name && typeof show.name === 'string') {
            try {
              await api.createShow(show.name);
              imported_count++;
            } catch (err) {
              console.error(`Failed to import "${show.name}":`, err);
            }
          }
        }

        toast.success(`Imported ${imported_count} show(s)`);
        fetchShows();
        event.target.value = ''; // Reset file input
      } catch (err) {
        toast.error('Failed to import shows: ' + err.message);
      }
    };
    reader.readAsText(file);
  }, [fetchShows]);

  const startEdit = useCallback((show) => {
    setEditingId(show.id);
    setEditingName(show.name);
  }, []);

  const cancelEdit = useCallback(() => {
    setEditingId(null);
    setEditingName('');
  }, []);

  // Filter and sort shows
  const filteredShows = useMemo(() => {
    let filtered = shows;
    
    // Apply search filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = shows.filter(show => show.name.toLowerCase().includes(query));
    }
    
    // Apply sorting
    const sorted = [...filtered].sort((a, b) => {
      switch (sortBy) {
        case 'name-asc':
          return a.name.localeCompare(b.name);
        case 'name-desc':
          return b.name.localeCompare(a.name);
        case 'date-asc':
          return a.id - b.id; // Older first (lower IDs)
        case 'date-desc':
          return b.id - a.id; // Newer first (higher IDs)
        case 'last-used-desc': {
          // Most recently used first, null values last
          if (!a.last_used_at && !b.last_used_at) return 0;
          if (!a.last_used_at) return 1;
          if (!b.last_used_at) return -1;
          return new Date(b.last_used_at) - new Date(a.last_used_at);
        }
        case 'last-used-asc': {
          // Least recently used first, null values last
          if (!a.last_used_at && !b.last_used_at) return 0;
          if (!a.last_used_at) return 1;
          if (!b.last_used_at) return -1;
          return new Date(a.last_used_at) - new Date(b.last_used_at);
        }
        default:
          return 0;
      }
    });
    
    return sorted;
  }, [shows, searchQuery, sortBy]);

  // Calculate pagination
  const totalPages = useMemo(() => {
    return Math.ceil(filteredShows.length / itemsPerPage);
  }, [filteredShows.length, itemsPerPage]);

  const paginatedShows = useMemo(() => {
    const startIndex = (currentPage - 1) * itemsPerPage;
    const endIndex = startIndex + itemsPerPage;
    return filteredShows.slice(startIndex, endIndex);
  }, [filteredShows, currentPage, itemsPerPage]);

  if (loading) {
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
          <h1 className="text-2xl md:text-3xl font-bold text-slate-100">TV Shows</h1>
          <p className="text-slate-400 mt-2 text-sm sm:text-base">
            Manage your list of shows. Select one to use as the default title for ripping.
          </p>
        </div>
        <div className="flex gap-2">
          {!isAdding && !bulkMode && (
            <>
              <input
                type="file"
                id="import-shows"
                accept=".json"
                onChange={handleImportShows}
                className="hidden"
              />
              <button
                onClick={() => document.getElementById('import-shows').click()}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors flex items-center whitespace-nowrap"
                title="Import shows from JSON"
              >
                <FontAwesomeIcon icon={faFileImport} className="mr-2" />
                Import
              </button>
              <button
                onClick={handleExportShows}
                disabled={shows.length === 0}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 disabled:text-slate-600 text-white rounded-lg transition-colors flex items-center whitespace-nowrap"
                title="Export shows to JSON"
              >
                <FontAwesomeIcon icon={faFileExport} className="mr-2" />
                Export
              </button>
              <button
                onClick={() => setBulkMode(true)}
                disabled={shows.length === 0}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 disabled:text-slate-600 text-white rounded-lg transition-colors flex items-center whitespace-nowrap"
              >
                <FontAwesomeIcon icon={faCheckSquare} className="mr-2" />
                Bulk
              </button>
              <button
                onClick={() => setIsAdding(true)}
                className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors flex items-center whitespace-nowrap"
              >
                <FontAwesomeIcon icon={faPlus} className="mr-2" />
                Add
              </button>
            </>
          )}
          {bulkMode && (
            <>
              <button
                onClick={handleBulkDelete}
                disabled={selectedShows.size === 0}
                className="px-4 py-2 bg-red-500 hover:bg-red-600 disabled:bg-slate-800 disabled:text-slate-600 text-white rounded-lg transition-colors flex items-center whitespace-nowrap"
              >
                <FontAwesomeIcon icon={faTrash} className="mr-2" />
                Delete ({selectedShows.size})
              </button>
              <button
                onClick={() => {
                  setBulkMode(false);
                  setSelectedShows(new Set());
                }}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
              >
                Cancel
              </button>
            </>
          )}
        </div>
      </div>

      {/* Search and Sort */}
      {shows.length > 0 && (
        <div className="flex flex-col sm:flex-row gap-3">
          <div className="relative flex-1">
            <FontAwesomeIcon
              icon={faSearch}
              className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400"
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search shows..."
              className="w-full pl-11 pr-4 py-3 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500 transition-colors"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-300"
              >
                <FontAwesomeIcon icon={faTimes} />
              </button>
            )}
          </div>
          <select
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value)}
            className="px-4 py-3 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500 transition-colors sm:w-auto w-full"
          >
            <option value="name-asc">Name (A-Z)</option>
            <option value="name-desc">Name (Z-A)</option>
            <option value="date-desc">Newest First</option>
            <option value="date-asc">Oldest First</option>
            <option value="last-used-desc">Recently Used</option>
            <option value="last-used-asc">Least Used</option>
          </select>
        </div>
      )}

      {/* Add New Show */}
      {isAdding && (
        <div className="bg-slate-800 rounded-lg p-5 border border-cyan-500">
          <h2 className="text-lg font-semibold text-slate-100 mb-3">Add New Show</h2>
          <div className="flex gap-2">
            <input
              type="text"
              value={newShowName}
              onChange={(e) => setNewShowName(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && handleAddShow()}
              placeholder="e.g., Foster's Home for Imaginary Friends"
              className="flex-1 px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
              autoFocus
            />
            <button
              onClick={handleAddShow}
              className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
            >
              <FontAwesomeIcon icon={faSave} className="mr-2" />
              Save
            </button>
            <button
              onClick={() => {
                setIsAdding(false);
                setNewShowName('');
              }}
              className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
            >
              <FontAwesomeIcon icon={faTimes} />
            </button>
          </div>
        </div>
      )}

      {/* Shows List */}
      {shows.length === 0 ? (
        <div className="bg-slate-800 rounded-lg p-8 border border-slate-700 text-center">
          <FontAwesomeIcon icon={faTv} className="text-slate-600 text-4xl mb-3" />
          <p className="text-slate-400">No shows added yet</p>
          <p className="text-slate-500 text-sm mt-2">Click "Add Show" to get started</p>
        </div>
      ) : filteredShows.length === 0 ? (
        <div className="bg-slate-800 rounded-lg p-8 border border-slate-700 text-center">
          <FontAwesomeIcon icon={faSearch} className="text-slate-600 text-4xl mb-3" />
          <p className="text-slate-400">No shows match your search</p>
          <p className="text-slate-500 text-sm mt-2">Try a different search term</p>
        </div>
      ) : (
        <>
          {bulkMode && (
            <div className="bg-slate-800 rounded-lg p-4 border border-slate-700 flex items-center justify-between">
              <button
                onClick={handleSelectAll}
                className="text-cyan-400 hover:text-cyan-300 transition-colors"
              >
                <FontAwesomeIcon icon={paginatedShows.length > 0 && paginatedShows.every(s => selectedShows.has(s.id)) ? faCheckSquare : faSquare} className="mr-2" />
                {paginatedShows.length > 0 && paginatedShows.every(s => selectedShows.has(s.id)) ? 'Deselect All' : 'Select All'}
              </button>
              <span className="text-slate-400 text-sm">
                {selectedShows.size} selected
              </span>
            </div>
          )}
          <div className="grid grid-cols-1 gap-3">
            {paginatedShows.map((show) => (
              <div
                key={show.id}
                className={`bg-slate-800 rounded-lg p-4 border transition-colors ${
                  selectedShowId === show.id
                    ? 'border-cyan-500 bg-cyan-500/5'
                    : selectedShows.has(show.id)
                    ? 'border-yellow-500 bg-yellow-500/5'
                    : 'border-slate-700 hover:border-slate-600'
                }`}
              >
                <div className="flex items-start gap-3">
                  {bulkMode && (
                    <button
                      onClick={() => handleToggleSelect(show.id)}
                      className="text-2xl text-slate-400 hover:text-cyan-400 transition-colors mt-1"
                    >
                      <FontAwesomeIcon icon={selectedShows.has(show.id) ? faCheckSquare : faSquare} />
                    </button>
                  )}
                  <div className="flex-1">
                    {editingId === show.id ? (
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={editingName}
                    onChange={(e) => setEditingName(e.target.value)}
                    onKeyPress={(e) => e.key === 'Enter' && handleUpdateShow(show.id)}
                    className="flex-1 px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
                    autoFocus
                  />
                  <button
                    onClick={() => handleUpdateShow(show.id)}
                    className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
                  >
                    <FontAwesomeIcon icon={faSave} />
                  </button>
                  <button
                    onClick={cancelEdit}
                    className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
                  >
                    <FontAwesomeIcon icon={faTimes} />
                  </button>
                </div>
              ) : (
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3 flex-1">
                    {selectedShowId === show.id && (
                      <FontAwesomeIcon
                        icon={faCheck}
                        className="text-cyan-400 text-lg"
                      />
                    )}
                    <div className="flex flex-col">
                      <span className="text-slate-100 font-medium text-lg">
                        {show.name}
                      </span>
                      <span className="text-slate-500 text-xs mt-0.5">
                        Last used: {formatRelativeTime(show.last_used_at)}
                      </span>
                    </div>
                  </div>
                  <div className="flex gap-2">
                    {selectedShowId !== show.id && (
                      <button
                        onClick={() => handleSelectShow(show.id)}
                        className="px-3 py-1.5 bg-cyan-600 hover:bg-cyan-500 text-white rounded text-sm transition-colors"
                      >
                        Select
                      </button>
                    )}
                    <button
                      onClick={() => startEdit(show)}
                      className="px-3 py-1.5 bg-slate-700 hover:bg-slate-600 text-white rounded text-sm transition-colors"
                    >
                      <FontAwesomeIcon icon={faEdit} />
                    </button>
                    <button
                      onClick={() => handleDeleteShow(show.id, show.name)}
                      className="px-3 py-1.5 bg-red-600 hover:bg-red-500 text-white rounded text-sm transition-colors"
                    >
                      <FontAwesomeIcon icon={faTrash} />
                    </button>
                  </div>
                </div>
              )}
                  </div>
                </div>
              </div>
            ))}
          </div>
          
          {/* Pagination Controls */}
          {filteredShows.length > 0 && (
            <div className="flex flex-col sm:flex-row items-center justify-between gap-4 mt-6 px-2">
              <div className="flex items-center gap-3">
                <span className="text-slate-400 text-sm">Items per page:</span>
                <select
                  value={itemsPerPage}
                  onChange={(e) => setItemsPerPage(Number(e.target.value))}
                  className="px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500 transition-colors"
                >
                  <option value={10}>10</option>
                  <option value={25}>25</option>
                  <option value={50}>50</option>
                  <option value={100}>100</option>
                </select>
                <span className="text-slate-400 text-sm">
                  Showing {Math.min((currentPage - 1) * itemsPerPage + 1, filteredShows.length)} - {Math.min(currentPage * itemsPerPage, filteredShows.length)} of {filteredShows.length}
                </span>
              </div>
              
              {totalPages > 1 && (
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                    disabled={currentPage === 1}
                    className="px-3 py-2 bg-slate-800 hover:bg-slate-700 disabled:bg-slate-900 disabled:text-slate-600 text-slate-100 rounded-lg border border-slate-700 transition-colors"
                    title="Previous page"
                  >
                    <FontAwesomeIcon icon={faChevronLeft} />
                  </button>
                  
                  <div className="flex items-center gap-1">
                    {Array.from({ length: Math.min(totalPages, 5) }, (_, i) => {
                      let pageNum;
                      if (totalPages <= 5) {
                        pageNum = i + 1;
                      } else if (currentPage <= 3) {
                        pageNum = i + 1;
                      } else if (currentPage >= totalPages - 2) {
                        pageNum = totalPages - 4 + i;
                      } else {
                        pageNum = currentPage - 2 + i;
                      }
                      
                      return (
                        <button
                          key={pageNum}
                          onClick={() => setCurrentPage(pageNum)}
                          className={`px-3 py-2 rounded-lg border transition-colors min-w-[40px] ${
                            currentPage === pageNum
                              ? 'bg-cyan-500 border-cyan-500 text-white'
                              : 'bg-slate-800 border-slate-700 text-slate-100 hover:bg-slate-700'
                          }`}
                        >
                          {pageNum}
                        </button>
                      );
                    })}
                  </div>
                  
                  <button
                    onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                    disabled={currentPage === totalPages}
                    className="px-3 py-2 bg-slate-800 hover:bg-slate-700 disabled:bg-slate-900 disabled:text-slate-600 text-slate-100 rounded-lg border border-slate-700 transition-colors"
                    title="Next page"
                  >
                    <FontAwesomeIcon icon={faChevronRight} />
                  </button>
                </div>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}
