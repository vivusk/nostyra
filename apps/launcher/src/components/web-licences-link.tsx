/** Web edition: the static licences page (public/licences.html, web build only).
 *  `import.meta.env.MODE` is replaced at build time, so the desktop bundle drops it. */
const LICENCES_URL = import.meta.env.MODE === "web" ? `${import.meta.env.BASE_URL}licences.html` : null;

/** "Licences and source code" link; renders nothing on the desktop. */
export function WebLicencesLink({ className = "" }: { className?: string }) {
  if (!LICENCES_URL) return null;
  return (
    <a href={LICENCES_URL} target="_blank" rel="noreferrer" className={`text-sm text-text underline ${className}`}>
      Licences and source code
    </a>
  );
}

/** Web first-run footer: privacy promise + licences link. */
export function WebPrivacyNote() {
  if (!LICENCES_URL) return null;
  return (
    <footer className="flex flex-wrap items-center justify-between gap-3 text-sm text-muted">
      <p className="m-0">Your ROMs and saves never leave your computer.</p>
      <WebLicencesLink />
    </footer>
  );
}
