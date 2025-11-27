import { useState } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faCircleQuestion } from '@fortawesome/free-solid-svg-icons';

export default function Tooltip({ text, icon = faCircleQuestion, placement = 'top' }) {
  const [show, setShow] = useState(false);

  const placementClasses = {
    top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
    bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
    left: 'right-full top-1/2 -translate-y-1/2 mr-2',
    right: 'left-full top-1/2 -translate-y-1/2 ml-2',
  };

  const arrowClasses = {
    top: 'top-full left-1/2 -translate-x-1/2 border-l-transparent border-r-transparent border-b-transparent border-t-slate-700',
    bottom: 'bottom-full left-1/2 -translate-x-1/2 border-l-transparent border-r-transparent border-t-transparent border-b-slate-700',
    left: 'left-full top-1/2 -translate-y-1/2 border-t-transparent border-b-transparent border-r-transparent border-l-slate-700',
    right: 'right-full top-1/2 -translate-y-1/2 border-t-transparent border-b-transparent border-l-transparent border-r-slate-700',
  };

  return (
    <div className="relative inline-block">
      <button
        type="button"
        onMouseEnter={() => setShow(true)}
        onMouseLeave={() => setShow(false)}
        onFocus={() => setShow(true)}
        onBlur={() => setShow(false)}
        className="text-slate-400 hover:text-slate-300 transition-colors"
      >
        <FontAwesomeIcon icon={icon} className="text-sm" />
      </button>

      {show && (
        <div
          className={`absolute ${placementClasses[placement]} z-50 px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg shadow-lg text-sm text-slate-200 whitespace-nowrap pointer-events-none`}
          style={{ maxWidth: '250px', whiteSpace: 'normal' }}
        >
          {text}
          <div
            className={`absolute w-0 h-0 border-4 ${arrowClasses[placement]}`}
          />
        </div>
      )}
    </div>
  );
}
