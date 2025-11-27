import { useState, useEffect, useCallback, useMemo } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faExclamationTriangle,
  faCircleCheck,
  faSpinner,
  faFilter,
  faTriangleExclamation,
  faCircleXmark,
  faBug,
  faCompactDisc,
  faNetworkWired,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';

export default function Issues() {
  const [issues, setIssues] = useState([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState('all'); // all, active, resolved

  useEffect(() => {
    fetchIssues();
  }, []);

  const fetchIssues = useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.getIssues();
      setIssues(data);
    } catch (err) {
      console.error('Failed to fetch issues:', err);
      toast.error('Failed to load issues');
    } finally {
      setLoading(false);
    }
  }, []);

  const handleResolveIssue = useCallback(async (issueId) => {
    try {
      await api.resolveIssue(issueId);
      toast.success('Issue resolved');
      fetchIssues();
    } catch (err) {
      toast.error('Failed to resolve issue: ' + err.message);
    }
  }, [fetchIssues]);

  const getIssueIcon = useCallback((issueType) => {
    switch (issueType?.toLowerCase()) {
      case 'drive_error':
        return faCompactDisc;
      case 'network_error':
        return faNetworkWired;
      case 'rip_error':
        return faCircleXmark;
      default:
        return faBug;
    }
  }, []);

  const getIssueColor = useCallback((issueType) => {
    switch (issueType?.toLowerCase()) {
      case 'drive_error':
        return 'text-yellow-400 bg-yellow-500/10 border-yellow-500/30';
      case 'network_error':
        return 'text-blue-400 bg-blue-500/10 border-blue-500/30';
      case 'rip_error':
        return 'text-red-400 bg-red-500/10 border-red-500/30';
      default:
        return 'text-orange-400 bg-orange-500/10 border-orange-500/30';
    }
  }, []);

  const filteredIssues = useMemo(() => {
    switch (filter) {
      case 'active':
        return issues.filter(i => !i.resolved);
      case 'resolved':
        return issues.filter(i => i.resolved);
      default:
        return issues;
    }
  }, [issues, filter]);

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
          <h1 className="text-3xl font-bold text-slate-100">Issues</h1>
          <p className="text-slate-400 mt-2">
            View and manage system issues and errors
          </p>
        </div>
        <button
          onClick={fetchIssues}
          className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
        >
          Refresh
        </button>
      </div>

      {/* Filter Tabs */}
      <div className="flex gap-2 border-b border-slate-700">
        <button
          onClick={() => setFilter('all')}
          className={`px-4 py-2 font-medium transition-colors ${
            filter === 'all'
              ? 'text-cyan-400 border-b-2 border-cyan-400'
              : 'text-slate-400 hover:text-slate-300'
          }`}
        >
          All Issues ({issues.length})
        </button>
        <button
          onClick={() => setFilter('active')}
          className={`px-4 py-2 font-medium transition-colors ${
            filter === 'active'
              ? 'text-cyan-400 border-b-2 border-cyan-400'
              : 'text-slate-400 hover:text-slate-300'
          }`}
        >
          Active ({issues.filter(i => !i.resolved).length})
        </button>
        <button
          onClick={() => setFilter('resolved')}
          className={`px-4 py-2 font-medium transition-colors ${
            filter === 'resolved'
              ? 'text-cyan-400 border-b-2 border-cyan-400'
              : 'text-slate-400 hover:text-slate-300'
          }`}
        >
          Resolved ({issues.filter(i => i.resolved).length})
        </button>
      </div>

      {/* Issues List */}
      {filteredIssues.length === 0 ? (
        <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
          <FontAwesomeIcon icon={faCircleCheck} className="text-green-400 text-5xl mb-4" />
          <h3 className="text-xl font-semibold text-slate-100 mb-2">
            {filter === 'active' ? 'No Active Issues' : filter === 'resolved' ? 'No Resolved Issues' : 'No Issues'}
          </h3>
          <p className="text-slate-400">
            {filter === 'active' 
              ? 'All systems running smoothly!' 
              : filter === 'resolved'
              ? 'No resolved issues to show'
              : 'No issues have been recorded'}
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {filteredIssues.map((issue) => (
            <div
              key={issue.id}
              className={`rounded-lg p-5 border ${getIssueColor(issue.issue_type)}`}
            >
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <div className="flex items-center mb-2">
                    <FontAwesomeIcon
                      icon={getIssueIcon(issue.issue_type)}
                      className="mr-3"
                    />
                    <h3 className="font-semibold text-lg">{issue.title}</h3>
                    <span className="ml-3 px-2 py-1 text-xs rounded bg-slate-900/50">
                      {issue.issue_type}
                    </span>
                    {issue.resolved && (
                      <span className="ml-2 px-2 py-1 text-xs rounded bg-green-500/20 text-green-400">
                        Resolved
                      </span>
                    )}
                  </div>
                  
                  <p className="text-slate-300 mb-3">{issue.description}</p>
                  
                  <div className="flex flex-wrap gap-4 text-sm text-slate-400">
                    <div>
                      <span className="font-medium">Time:</span>{' '}
                      {new Date(issue.timestamp).toLocaleString()}
                    </div>
                    {issue.drive && (
                      <div>
                        <span className="font-medium">Drive:</span> {issue.drive}
                      </div>
                    )}
                    {issue.disc && (
                      <div>
                        <span className="font-medium">Disc:</span> {issue.disc}
                      </div>
                    )}
                    {issue.resolved_at && (
                      <div>
                        <span className="font-medium">Resolved:</span>{' '}
                        {new Date(issue.resolved_at).toLocaleString()}
                      </div>
                    )}
                  </div>
                </div>
                
                {!issue.resolved && (
                  <button
                    onClick={() => handleResolveIssue(issue.id)}
                    className="ml-4 px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm rounded transition-colors flex items-center"
                  >
                    <FontAwesomeIcon icon={faCircleCheck} className="mr-2" />
                    Resolve
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
