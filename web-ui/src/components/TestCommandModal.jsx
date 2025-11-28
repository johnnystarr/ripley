import { useState, useRef, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faTimes, faPlay, faTrash } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import toast from 'react-hot-toast';

export default function TestCommandModal({ agentId, agentName, isOpen, onClose }) {
  const [command, setCommand] = useState('');
  const [logs, setLogs] = useState([]);
  const [isRunning, setIsRunning] = useState(false);
  const logsEndRef = useRef(null);
  const pollIntervalRef = useRef(null);
  const pollTimeoutRef = useRef(null);

  useEffect(() => {
    if (isOpen) {
      setCommand('');
      setLogs([]);
      setIsRunning(false);
    } else {
      // Clean up polling when modal closes
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (pollTimeoutRef.current) {
        clearTimeout(pollTimeoutRef.current);
        pollTimeoutRef.current = null;
      }
    }
    
    return () => {
      // Cleanup on unmount
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
      if (pollTimeoutRef.current) {
        clearTimeout(pollTimeoutRef.current);
      }
    };
  }, [isOpen]);

  useEffect(() => {
    // Auto-scroll to bottom when logs update
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const handleRunCommand = async () => {
    if (!command.trim()) {
      toast.error('Please enter a command');
      return;
    }

    const commandToRun = command.trim();
    const timestamp = new Date().toLocaleTimeString();
    
    // Add command to logs
    setLogs(prev => [...prev, {
      type: 'command',
      timestamp,
      content: commandToRun,
    }]);

    setIsRunning(true);
    setCommand(''); // Clear input for next command

    try {
      const result = await api.testAgentCommand(agentId, commandToRun);
      const instructionId = result.instruction_id;
      
      // Poll for instruction result
      pollIntervalRef.current = setInterval(async () => {
        try {
          const instruction = await api.getInstruction(instructionId);
          
          if (instruction.status === 'completed' || instruction.status === 'failed') {
            if (pollIntervalRef.current) {
              clearInterval(pollIntervalRef.current);
              pollIntervalRef.current = null;
            }
            if (pollTimeoutRef.current) {
              clearTimeout(pollTimeoutRef.current);
              pollTimeoutRef.current = null;
            }
            setIsRunning(false);
            
            if (instruction.status === 'completed' && instruction.output) {
              // Add output to logs
              setLogs(prev => [...prev, {
                type: 'output',
                timestamp: new Date().toLocaleTimeString(),
                content: instruction.output,
              }]);
            } else if (instruction.status === 'failed') {
              // Add error to logs
              setLogs(prev => [...prev, {
                type: 'error',
                timestamp: new Date().toLocaleTimeString(),
                content: instruction.error_message || 'Command failed',
              }]);
            }
          }
        } catch (err) {
          // Continue polling on error
        }
      }, 500); // Poll every 500ms
      
      // Stop polling after 30 seconds
      pollTimeoutRef.current = setTimeout(() => {
        if (pollIntervalRef.current) {
          clearInterval(pollIntervalRef.current);
          pollIntervalRef.current = null;
        }
        setIsRunning(false);
      }, 30000);
      
    } catch (err) {
      setIsRunning(false);
      // Add error message
      setLogs(prev => [...prev, {
        type: 'error',
        timestamp: new Date().toLocaleTimeString(),
        content: `Error: ${err.message}`,
      }]);
    }
  };

  const handleClearLogs = () => {
    setLogs([]);
  };

  const handleKeyPress = (e) => {
    if (e.key === 'Enter' && !e.shiftKey && !isRunning) {
      e.preventDefault();
      handleRunCommand();
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="bg-slate-800 rounded-lg border border-slate-700 shadow-2xl w-full max-w-4xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-slate-700">
          <div>
            <h2 className="text-2xl font-bold text-slate-100">Test Agent Command</h2>
            <p className="text-sm text-slate-400 mt-1">Agent: {agentName}</p>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-200 transition-colors p-2 hover:bg-slate-700 rounded"
            title="Close"
          >
            <FontAwesomeIcon icon={faTimes} className="text-xl" />
          </button>
        </div>

        {/* Command Input */}
        <div className="p-6 border-b border-slate-700">
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Command to Run
          </label>
          <div className="flex gap-2">
            <textarea
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              onKeyPress={handleKeyPress}
              placeholder="Enter command to test (e.g., echo 'Hello World', dir, ls, etc.)"
              className="flex-1 px-4 py-2 bg-slate-900 border border-slate-600 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500 font-mono text-sm resize-none"
              rows="2"
              disabled={isRunning}
            />
            <button
              onClick={handleRunCommand}
              disabled={isRunning || !command.trim()}
              className="px-6 py-2 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap"
            >
              <FontAwesomeIcon icon={faPlay} />
              Run
            </button>
          </div>
          <p className="text-xs text-slate-500 mt-2">
            Press Enter to run, Shift+Enter for new line. Commands are sent to the agent in real-time.
          </p>
        </div>

        {/* Log Output */}
        <div className="flex-1 overflow-hidden flex flex-col p-6">
          <div className="flex items-center justify-between mb-3">
            <label className="block text-sm font-medium text-slate-300">
              Output Log
            </label>
            {logs.length > 0 && (
              <button
                onClick={handleClearLogs}
                className="text-xs text-slate-400 hover:text-slate-200 transition-colors flex items-center gap-1"
              >
                <FontAwesomeIcon icon={faTrash} />
                Clear
              </button>
            )}
          </div>
          <div className="flex-1 bg-slate-900 rounded-lg border border-slate-700 p-4 overflow-y-auto font-mono text-sm">
            {logs.length === 0 ? (
              <div className="text-slate-500 text-center py-8">
                No commands run yet. Enter a command above and click Run.
              </div>
            ) : (
              <div className="space-y-2">
                {logs.map((log, index) => (
                  <div
                    key={index}
                    className={`${
                      log.type === 'command'
                        ? 'text-cyan-400'
                        : log.type === 'output'
                        ? 'text-green-400'
                        : log.type === 'error'
                        ? 'text-red-400'
                        : 'text-slate-300'
                    } whitespace-pre-wrap break-words`}
                  >
                    <span className="text-slate-500">[{log.timestamp}] </span>
                    {log.type === 'command' && <span className="text-slate-400">$ </span>}
                    {log.content}
                  </div>
                ))}
                {isRunning && (
                  <div className="text-slate-500 text-sm italic">
                    Waiting for command output...
                  </div>
                )}
                <div ref={logsEndRef} />
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-slate-700 flex justify-end">
          <button
            onClick={onClose}
            className="px-6 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
