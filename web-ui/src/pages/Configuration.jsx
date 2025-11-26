import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faSave,
  faSpinner,
  faKey,
  faBell,
  faSync,
  faFilm,
  faMicrophone,
  faEye,
  faEyeSlash,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';

export default function Configuration() {
  const [config, setConfig] = useState(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [showOpenAIKey, setShowOpenAIKey] = useState(false);
  const [showTMDBKey, setShowTMDBKey] = useState(false);

  useEffect(() => {
    fetchConfig();
  }, []);

  const fetchConfig = async () => {
    try {
      setLoading(true);
      const data = await api.getConfig();
      setConfig(data);
    } catch (err) {
      toast.error('Failed to load configuration: ' + err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      await api.updateConfig(config);
      toast.success('Configuration saved successfully!');
    } catch (err) {
      toast.error('Failed to save configuration: ' + err.message);
    } finally {
      setSaving(false);
    }
  };

  const updateConfig = (path, value) => {
    setConfig(prev => {
      const newConfig = { ...prev };
      const keys = path.split('.');
      let current = newConfig;
      
      for (let i = 0; i < keys.length - 1; i++) {
        current = current[keys[i]];
      }
      
      current[keys[keys.length - 1]] = value;
      return newConfig;
    });
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  if (!config) {
    return (
      <div className="bg-red-500/10 border border-red-500 text-red-400 px-4 py-3 rounded-lg">
        Failed to load configuration
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-slate-100">Configuration</h1>
        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center px-4 py-2 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors"
        >
          {saving ? (
            <>
              <FontAwesomeIcon icon={faSpinner} className="mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <FontAwesomeIcon icon={faSave} className="mr-2" />
              Save Configuration
            </>
          )}
        </button>
      </div>

      {/* API Keys Section */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4 flex items-center">
          <FontAwesomeIcon icon={faKey} className="mr-2 text-cyan-400" />
          API Keys
        </h2>
        
        <div className="space-y-4">
          {/* OpenAI API Key */}
          <div>
            <label className="block text-slate-400 text-sm mb-2">
              OpenAI API Key
              <span className="text-slate-500 ml-2">(for speech matching)</span>
            </label>
            <div className="relative">
              <input
                type={showOpenAIKey ? 'text' : 'password'}
                value={config.openai_api_key || ''}
                onChange={(e) => updateConfig('openai_api_key', e.target.value || null)}
                placeholder="sk-..."
                className="w-full px-4 py-2 pr-12 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
              />
              <button
                type="button"
                onClick={() => setShowOpenAIKey(!showOpenAIKey)}
                className="absolute right-3 top-1/2 transform -translate-y-1/2 text-slate-400 hover:text-slate-300"
              >
                <FontAwesomeIcon icon={showOpenAIKey ? faEyeSlash : faEye} />
              </button>
            </div>
          </div>

          {/* TMDB API Key */}
          <div>
            <label className="block text-slate-400 text-sm mb-2">
              TMDB API Key
              <span className="text-slate-500 ml-2">(for movie metadata)</span>
            </label>
            <div className="relative">
              <input
                type={showTMDBKey ? 'text' : 'password'}
                value={config.tmdb_api_key || ''}
                onChange={(e) => updateConfig('tmdb_api_key', e.target.value || null)}
                placeholder="Enter TMDB API key"
                className="w-full px-4 py-2 pr-12 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
              />
              <button
                type="button"
                onClick={() => setShowTMDBKey(!showTMDBKey)}
                className="absolute right-3 top-1/2 transform -translate-y-1/2 text-slate-400 hover:text-slate-300"
              >
                <FontAwesomeIcon icon={showTMDBKey ? faEyeSlash : faEye} />
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Notifications Section */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4 flex items-center">
          <FontAwesomeIcon icon={faBell} className="mr-2 text-cyan-400" />
          Notifications
        </h2>
        
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Enable Notifications</label>
              <p className="text-slate-400 text-sm mt-1">Send push notifications for rip events</p>
            </div>
            <button
              onClick={() => updateConfig('notifications.enabled', !config.notifications.enabled)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.notifications.enabled ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.notifications.enabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">Notification Topic</label>
            <input
              type="text"
              value={config.notifications.topic}
              onChange={(e) => updateConfig('notifications.topic', e.target.value)}
              placeholder="notification_topic"
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
            />
          </div>
        </div>
      </div>

      {/* Rsync Section */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4 flex items-center">
          <FontAwesomeIcon icon={faSync} className="mr-2 text-cyan-400" />
          Rsync Backup
        </h2>
        
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Enable Rsync</label>
              <p className="text-slate-400 text-sm mt-1">Automatically backup ripped files</p>
            </div>
            <button
              onClick={() => updateConfig('rsync.enabled', !config.rsync.enabled)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.rsync.enabled ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.rsync.enabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">Destination Path</label>
            <input
              type="text"
              value={config.rsync.destination}
              onChange={(e) => updateConfig('rsync.destination', e.target.value)}
              placeholder="/path/to/backup"
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
            />
          </div>
        </div>
      </div>

      {/* Speech Match Section */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4 flex items-center">
          <FontAwesomeIcon icon={faMicrophone} className="mr-2 text-cyan-400" />
          Speech Matching
        </h2>
        
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Enable Speech Match</label>
              <p className="text-slate-400 text-sm mt-1">Use Whisper to identify episode titles</p>
            </div>
            <button
              onClick={() => updateConfig('speech_match.enabled', !config.speech_match.enabled)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.speech_match.enabled ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.speech_match.enabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">
              Audio Duration (seconds)
              <span className="text-slate-500 ml-2">{config.speech_match.audio_duration}s</span>
            </label>
            <input
              type="range"
              min="30"
              max="600"
              value={config.speech_match.audio_duration}
              onChange={(e) => updateConfig('speech_match.audio_duration', parseInt(e.target.value))}
              className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-cyan-500"
            />
            <div className="flex justify-between text-xs text-slate-500 mt-1">
              <span>30s</span>
              <span>10min</span>
            </div>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">Whisper Model</label>
            <select
              value={config.speech_match.whisper_model}
              onChange={(e) => updateConfig('speech_match.whisper_model', e.target.value)}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="tiny">Tiny (fastest, least accurate)</option>
              <option value="base">Base (recommended)</option>
              <option value="small">Small (slower, more accurate)</option>
              <option value="medium">Medium (very slow)</option>
              <option value="large">Large (slowest, best accuracy)</option>
            </select>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Use OpenAI API</label>
              <p className="text-slate-400 text-sm mt-1">Use cloud API instead of local Whisper</p>
            </div>
            <button
              onClick={() => updateConfig('speech_match.use_openai_api', !config.speech_match.use_openai_api)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.speech_match.use_openai_api ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.speech_match.use_openai_api ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>
        </div>
      </div>

      {/* Filebot Section */}
      <div className="bg-slate-800 rounded-lg p-6 border border-slate-700">
        <h2 className="text-xl font-semibold text-slate-100 mb-4 flex items-center">
          <FontAwesomeIcon icon={faFilm} className="mr-2 text-cyan-400" />
          Filebot
        </h2>
        
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Skip by Default</label>
              <p className="text-slate-400 text-sm mt-1">Skip Filebot processing unless specified</p>
            </div>
            <button
              onClick={() => updateConfig('filebot.skip_by_default', !config.filebot.skip_by_default)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.filebot.skip_by_default ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.filebot.skip_by_default ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">Database</label>
            <select
              value={config.filebot.database}
              onChange={(e) => updateConfig('filebot.database', e.target.value)}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="TheTVDB">TheTVDB</option>
              <option value="TheMovieDB">TheMovieDB</option>
            </select>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">Episode Order</label>
            <select
              value={config.filebot.order}
              onChange={(e) => updateConfig('filebot.order', e.target.value)}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="Airdate">Airdate</option>
              <option value="DVD">DVD</option>
            </select>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Use for Music</label>
              <p className="text-slate-400 text-sm mt-1">Standardize music file names with Filebot</p>
            </div>
            <button
              onClick={() => updateConfig('filebot.use_for_music', !config.filebot.use_for_music)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.filebot.use_for_music ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.filebot.use_for_music ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>
        </div>
      </div>

      {/* Save Button (bottom) */}
      <div className="flex justify-end">
        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center px-6 py-3 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors text-lg"
        >
          {saving ? (
            <>
              <FontAwesomeIcon icon={faSpinner} className="mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <FontAwesomeIcon icon={faSave} className="mr-2" />
              Save Configuration
            </>
          )}
        </button>
      </div>
    </div>
  );
}
