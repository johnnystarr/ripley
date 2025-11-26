import { useState, useEffect, useCallback } from 'react';
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
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';

export default function Shows() {
  const [shows, setShows] = useState([]);
  const [loading, setLoading] = useState(true);
  const [newShowName, setNewShowName] = useState('');
  const [isAdding, setIsAdding] = useState(false);
  const [editingId, setEditingId] = useState(null);
  const [editingName, setEditingName] = useState('');
  const [selectedShowId, setSelectedShowId] = useState(null);

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
      toast.success('Show selected - will be used for all rips');
    } catch (err) {
      toast.error('Failed to select show: ' + err.message);
    }
  }, []);

  const startEdit = useCallback((show) => {
    setEditingId(show.id);
    setEditingName(show.name);
  }, []);

  const cancelEdit = useCallback(() => {
    setEditingId(null);
    setEditingName('');
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-slate-100">TV Shows</h1>
          <p className="text-slate-400 mt-2">
            Manage your list of shows. Select one to use as the default title for ripping.
          </p>
        </div>
        {!isAdding && (
          <button
            onClick={() => setIsAdding(true)}
            className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors flex items-center"
          >
            <FontAwesomeIcon icon={faPlus} className="mr-2" />
            Add Show
          </button>
        )}
      </div>

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
      ) : (
        <div className="grid grid-cols-1 gap-3">
          {shows.map((show) => (
            <div
              key={show.id}
              className={`bg-slate-800 rounded-lg p-4 border transition-colors ${
                selectedShowId === show.id
                  ? 'border-cyan-500 bg-cyan-500/5'
                  : 'border-slate-700 hover:border-slate-600'
              }`}
            >
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
                    <span className="text-slate-100 font-medium text-lg">
                      {show.name}
                    </span>
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
          ))}
        </div>
      )}
    </div>
  );
}
