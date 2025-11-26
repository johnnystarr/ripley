import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faCompactDisc,
  faPlay,
  faSync,
  faSpinner,
  faMusic,
  faFilm,
  faCircle,
} from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';

export default function Drives() {
  const [drives, setDrives] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    fetchDrives();
    const interval = setInterval(fetchDrives, 5000); // Refresh every 5 seconds
    return () => clearInterval(interval);
  }, []);

  const fetchDrives = async () => {
    try {
      const data = await api.getDrives();
      setDrives(data);
      setError(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  const handleRefresh = () => {
    setRefreshing(true);
    fetchDrives();
  };

  const handleStartRip = async (drive) => {
    try {
      await api.startRip({
        output_path: null, // Use default
        title: null, // Will prompt or auto-detect
        skip_metadata: false,
        skip_filebot: false,
      });
      // Redirect to dashboard to see progress
      window.location.href = '/';
    } catch (err) {
      setError(err.message);
    }
  };

  const getMediaIcon = (mediaType) => {
    switch (mediaType) {
      case 'AudioCD':
        return faMusic;
      case 'DVD':
      case 'BluRay':
        return faFilm;
      default:
        return faCompactDisc;
    }
  };

  const getMediaColor = (mediaType) => {
    switch (mediaType) {
      case 'AudioCD':
        return 'text-cyan-400';
      case 'DVD':
        return 'text-blue-400';
      case 'BluRay':
        return 'text-purple-400';
      default:
        return 'text-slate-400';
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
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-slate-100">Optical Drives</h1>
        <button
          onClick={handleRefresh}
          disabled={refreshing}
          className="flex items-center px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 text-white rounded-lg transition-colors"
        >
          <FontAwesomeIcon 
            icon={faSync} 
            className={`mr-2 ${refreshing ? 'animate-spin' : ''}`} 
          />
          Refresh
        </button>
      </div>

      {error && (
        <div className="bg-red-500/10 border border-red-500 text-red-400 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      {drives.length === 0 ? (
        <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
          <FontAwesomeIcon icon={faCompactDisc} className="text-slate-600 text-6xl mb-4" />
          <h2 className="text-xl font-semibold text-slate-300 mb-2">No Drives Detected</h2>
          <p className="text-slate-400">
            No optical drives were found on this system.
          </p>
        </div>
      ) : (
        <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          {drives.map((drive, index) => (
            <div
              key={index}
              className="bg-slate-800 rounded-lg p-6 border border-slate-700 hover:border-cyan-500/50 transition-colors"
            >
              <div className="flex items-start justify-between mb-4">
                <div className="flex items-center">
                  <FontAwesomeIcon
                    icon={getMediaIcon(drive.media_type)}
                    className={`text-3xl mr-3 ${getMediaColor(drive.media_type)}`}
                  />
                  <div>
                    <h3 className="text-lg font-semibold text-slate-100">
                      {drive.name}
                    </h3>
                    <p className="text-sm text-slate-400 font-mono">{drive.device}</p>
                  </div>
                </div>
              </div>

              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-slate-400 text-sm">Media Type:</span>
                  <span className={`px-3 py-1 rounded-full text-sm font-medium ${
                    drive.media_type !== 'None'
                      ? 'bg-green-500/20 text-green-400'
                      : 'bg-slate-700 text-slate-400'
                  }`}>
                    {drive.media_type === 'None' ? 'Empty' : drive.media_type}
                  </span>
                </div>

                {drive.media_type !== 'None' && (
                  <button
                    onClick={() => handleStartRip(drive)}
                    className="w-full flex items-center justify-center px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
                  >
                    <FontAwesomeIcon icon={faPlay} className="mr-2" />
                    Start Rip
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="bg-slate-800 rounded-lg p-4 border border-slate-700">
        <div className="flex items-start">
          <FontAwesomeIcon icon={faCircle} className="text-cyan-400 text-xs mt-1 mr-2" />
          <div className="text-sm text-slate-400">
            <p className="font-medium text-slate-300 mb-1">Auto-refresh enabled</p>
            <p>Drive status updates every 5 seconds automatically.</p>
          </div>
        </div>
      </div>
    </div>
  );
}
