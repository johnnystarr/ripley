import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faSave, faSpinner, faCog } from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';
import Tooltip from '../components/Tooltip';

export default function Preferences() {
  const [preferences, setPreferences] = useState(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    fetchPreferences();
  }, []);

  const fetchPreferences = async () => {
    try {
      setLoading(true);
      const data = await api.getPreferences();
      setPreferences(data);
    } catch (err) {
      toast.error('Failed to load preferences: ' + err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      await api.updatePreferences(preferences);
      toast.success('Preferences saved successfully!');
    } catch (err) {
      toast.error('Failed to save preferences: ' + err.message);
    } finally {
      setSaving(false);
    }
  };

  const updatePreference = (key, value) => {
    setPreferences(prev => ({
      ...prev,
      [key]: value,
    }));
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  if (!preferences) {
    return (
      <div className="bg-red-500/10 border border-red-500 text-red-400 px-4 py-3 rounded-lg">
        Failed to load preferences
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <h1 className="text-2xl md:text-3xl font-bold text-slate-100">Preferences</h1>
        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center justify-center px-4 py-2 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors whitespace-nowrap"
        >
          {saving ? (
            <>
              <FontAwesomeIcon icon={faSpinner} className="mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <FontAwesomeIcon icon={faSave} className="mr-2" />
              Save
            </>
          )}
        </button>
      </div>

      <div className="bg-slate-800 rounded-lg border border-slate-700">
        <div className="p-6 space-y-6">
          <div className="flex items-center gap-3 pb-4 border-b border-slate-700">
            <FontAwesomeIcon icon={faCog} className="text-cyan-400 text-xl" />
            <h2 className="text-xl font-semibold text-slate-100">User Interface</h2>
          </div>

          {/* Logs per page */}
          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              Logs per page
              <Tooltip text="Number of log entries to display per page on the Logs page" />
            </label>
            <select
              value={preferences.logs_per_page}
              onChange={(e) => updatePreference('logs_per_page', parseInt(e.target.value))}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value={25}>25</option>
              <option value={50}>50</option>
              <option value={100}>100</option>
              <option value={200}>200</option>
              <option value={500}>500</option>
            </select>
          </div>

          {/* Polling interval */}
          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              Status polling interval
              <Tooltip text="How often to check for status updates (in milliseconds). Lower values provide more real-time updates but use more resources." />
            </label>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min="1000"
                max="10000"
                step="500"
                value={preferences.polling_interval_ms}
                onChange={(e) => updatePreference('polling_interval_ms', parseInt(e.target.value))}
                className="flex-1"
              />
              <span className="text-slate-100 font-mono w-24 text-right">
                {preferences.polling_interval_ms}ms
              </span>
            </div>
            <div className="flex justify-between text-xs text-slate-500 mt-1">
              <span>1s (faster)</span>
              <span>10s (slower)</span>
            </div>
          </div>

          {/* Sound notifications */}
          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              Sound notifications
              <Tooltip text="Play a sound when rip operations complete successfully" />
            </label>
            <div className="flex items-center gap-3">
              <button
                onClick={() => updatePreference('sound_notifications', !preferences.sound_notifications)}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                  preferences.sound_notifications ? 'bg-cyan-500' : 'bg-slate-700'
                }`}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                    preferences.sound_notifications ? 'translate-x-6' : 'translate-x-1'
                  }`}
                />
              </button>
              <span className="text-slate-300 text-sm">
                {preferences.sound_notifications ? 'Enabled' : 'Disabled'}
              </span>
            </div>
          </div>

          {/* Theme (future feature) */}
          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              Theme
              <Tooltip text="Visual theme for the interface. Light theme coming soon!" />
            </label>
            <select
              value={preferences.theme}
              onChange={(e) => updatePreference('theme', e.target.value)}
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
            >
              <option value="dark">Dark</option>
              <option value="light" disabled>Light (Coming Soon)</option>
            </select>
          </div>
        </div>
      </div>

      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
        <p className="text-slate-400 text-sm">
          💡 <strong>Tip:</strong> Changes to polling interval and logs per page take effect after saving and refreshing the page.
        </p>
      </div>
    </div>
  );
}
