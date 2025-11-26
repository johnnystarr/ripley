import { Component } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faExclamationTriangle, faRotateRight } from '@fortawesome/free-solid-svg-icons';

class ErrorBoundary extends Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error) {
    return { hasError: true };
  }

  componentDidCatch(error, errorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    this.setState({
      error,
      errorInfo
    });
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null, errorInfo: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen bg-slate-900 flex items-center justify-center p-4">
          <div className="bg-slate-800 rounded-lg border border-red-500/50 p-8 max-w-2xl w-full">
            <div className="flex items-center mb-4">
              <FontAwesomeIcon 
                icon={faExclamationTriangle} 
                className="text-red-500 text-3xl mr-4" 
              />
              <h1 className="text-2xl font-bold text-slate-100">Something went wrong</h1>
            </div>
            
            <p className="text-slate-300 mb-4">
              The application encountered an unexpected error. This has been logged and we apologize for the inconvenience.
            </p>

            {this.state.error && (
              <div className="bg-slate-900 rounded p-4 mb-4 border border-slate-700">
                <p className="text-red-400 font-mono text-sm mb-2">
                  {this.state.error.toString()}
                </p>
                {this.state.errorInfo && (
                  <details className="mt-2">
                    <summary className="text-slate-400 text-sm cursor-pointer hover:text-slate-300">
                      Stack trace
                    </summary>
                    <pre className="text-xs text-slate-500 mt-2 overflow-x-auto">
                      {this.state.errorInfo.componentStack}
                    </pre>
                  </details>
                )}
              </div>
            )}

            <div className="flex gap-3">
              <button
                onClick={this.handleReset}
                className="bg-cyan-600 hover:bg-cyan-500 text-white px-4 py-2 rounded transition-colors duration-200 flex items-center"
              >
                <FontAwesomeIcon icon={faRotateRight} className="mr-2" />
                Try Again
              </button>
              <button
                onClick={() => window.location.href = '/'}
                className="bg-slate-700 hover:bg-slate-600 text-slate-200 px-4 py-2 rounded transition-colors duration-200"
              >
                Return to Dashboard
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
