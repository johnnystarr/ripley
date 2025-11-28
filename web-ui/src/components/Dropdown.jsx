import { useState, useRef, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronDown, faCheck } from '@fortawesome/free-solid-svg-icons';

export default function Dropdown({ label, value, options, onChange, className = '' }) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef(null);

  useEffect(() => {
    function handleClickOutside(event) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target)) {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  const selectedOption = options.find(opt => opt.value === value);

  return (
    <div className={`relative ${className}`} ref={dropdownRef}>
      {label && (
        <label className="block text-sm font-medium text-slate-300 mb-2">
          {label}
        </label>
      )}
      
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2.5 text-left text-slate-200 hover:border-slate-500 focus:border-cyan-500 focus:ring-2 focus:ring-cyan-500/20 transition-colors flex items-center justify-between"
      >
        <span>{selectedOption?.label || 'Select...'}</span>
        <FontAwesomeIcon 
          icon={faChevronDown} 
          className={`text-slate-400 transition-transform ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {isOpen && (
        <div className="absolute z-50 w-full mt-2 bg-slate-800 border border-slate-600 rounded-lg shadow-xl overflow-hidden">
          <div className="max-h-60 overflow-y-auto">
            {options.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => {
                  if (!option.disabled) {
                    onChange(option.value);
                    setIsOpen(false);
                  }
                }}
                disabled={option.disabled}
                className={`w-full px-4 py-2.5 text-left transition-colors flex items-center justify-between ${
                  option.disabled
                    ? 'text-slate-500 cursor-not-allowed opacity-50'
                    : value === option.value
                    ? 'bg-cyan-500/10 text-cyan-400 hover:bg-slate-700'
                    : 'text-slate-200 hover:bg-slate-700'
                }`}
              >
                <span>{option.label}</span>
                {value === option.value && !option.disabled && (
                  <FontAwesomeIcon icon={faCheck} className="text-cyan-400" />
                )}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
