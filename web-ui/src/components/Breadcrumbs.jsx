import { Link, useLocation } from 'react-router-dom';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronRight, faHome } from '@fortawesome/free-solid-svg-icons';

const routeNames = {
  '/': 'Dashboard',
  '/shows': 'Shows',
  '/issues': 'Issues',
  '/configuration': 'Configuration',
  '/preferences': 'Preferences',
  '/logs': 'Logs',
};

export default function Breadcrumbs() {
  const location = useLocation();
  const pathnames = location.pathname.split('/').filter((x) => x);

  // Don't show breadcrumbs on home page
  if (location.pathname === '/') {
    return null;
  }

  return (
    <nav className="flex items-center space-x-2 text-sm mb-6">
      <Link
        to="/"
        className="flex items-center text-slate-400 hover:text-cyan-400 transition-colors"
      >
        <FontAwesomeIcon icon={faHome} className="text-xs" />
      </Link>

      {pathnames.map((name, index) => {
        const routeTo = `/${pathnames.slice(0, index + 1).join('/')}`;
        const isLast = index === pathnames.length - 1;
        const displayName = routeNames[routeTo] || name.charAt(0).toUpperCase() + name.slice(1);

        return (
          <div key={routeTo} className="flex items-center space-x-2">
            <FontAwesomeIcon icon={faChevronRight} className="text-slate-600 text-xs" />
            {isLast ? (
              <span className="text-slate-100 font-medium">{displayName}</span>
            ) : (
              <Link
                to={routeTo}
                className="text-slate-400 hover:text-cyan-400 transition-colors"
              >
                {displayName}
              </Link>
            )}
          </div>
        );
      })}
    </nav>
  );
}
