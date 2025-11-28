import { Fragment } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faExclamationTriangle,
  faCheckCircle,
  faInfoCircle,
  faTimes,
} from '@fortawesome/free-solid-svg-icons';

/**
 * Reusable confirmation modal component
 * @param {boolean} isOpen - Whether the modal is visible
 * @param {string} title - Modal title
 * @param {string|ReactNode} message - Modal message/content
 * @param {string} type - Modal type: 'danger', 'warning', 'info', 'success'
 * @param {string} confirmText - Text for confirm button (default: 'Confirm')
 * @param {string} cancelText - Text for cancel button (default: 'Cancel')
 * @param {function} onConfirm - Callback when confirm is clicked
 * @param {function} onCancel - Callback when cancel is clicked or modal is closed
 * @param {boolean} showCancel - Whether to show cancel button (default: true)
 */
export default function ConfirmModal({
  isOpen,
  title,
  message,
  type = 'warning',
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  onConfirm,
  onCancel,
  showCancel = true,
}) {
  if (!isOpen) return null;

  const typeStyles = {
    danger: {
      icon: faExclamationTriangle,
      iconColor: 'text-red-400',
      iconBg: 'bg-red-500/10',
      iconBorder: 'border-red-500/30',
      button: 'bg-red-600 hover:bg-red-700',
      titleColor: 'text-red-400',
    },
    warning: {
      icon: faExclamationTriangle,
      iconColor: 'text-yellow-400',
      iconBg: 'bg-yellow-500/10',
      iconBorder: 'border-yellow-500/30',
      button: 'bg-yellow-600 hover:bg-yellow-700',
      titleColor: 'text-yellow-400',
    },
    info: {
      icon: faInfoCircle,
      iconColor: 'text-blue-400',
      iconBg: 'bg-blue-500/10',
      iconBorder: 'border-blue-500/30',
      button: 'bg-blue-600 hover:bg-blue-700',
      titleColor: 'text-blue-400',
    },
    success: {
      icon: faCheckCircle,
      iconColor: 'text-green-400',
      iconBg: 'bg-green-500/10',
      iconBorder: 'border-green-500/30',
      button: 'bg-green-600 hover:bg-green-700',
      titleColor: 'text-green-400',
    },
  };

  const styles = typeStyles[type] || typeStyles.warning;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        onClick={onCancel}
      />

      {/* Modal */}
      <div className="relative bg-slate-800 border border-slate-700 rounded-lg shadow-2xl max-w-md w-full mx-4 z-10">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-slate-700">
          <div className="flex items-center gap-3">
            <div
              className={`flex items-center justify-center w-10 h-10 rounded-full ${styles.iconBg} border ${styles.iconBorder}`}
            >
              <FontAwesomeIcon
                icon={styles.icon}
                className={`${styles.iconColor} text-xl`}
              />
            </div>
            <h3 className={`text-lg font-semibold ${styles.titleColor}`}>
              {title}
            </h3>
          </div>
          <button
            onClick={onCancel}
            className="text-slate-400 hover:text-slate-200 transition-colors"
          >
            <FontAwesomeIcon icon={faTimes} />
          </button>
        </div>

        {/* Body */}
        <div className="p-4">
          <div className="text-slate-300 whitespace-pre-line">{message}</div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 p-4 border-t border-slate-700">
          {showCancel && (
            <button
              onClick={onCancel}
              className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
            >
              {cancelText}
            </button>
          )}
          <button
            onClick={onConfirm}
            className={`px-4 py-2 ${styles.button} text-white rounded-lg transition-colors font-medium`}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
