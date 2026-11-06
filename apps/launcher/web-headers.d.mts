// Types of web-headers.mjs (plain JS so scripts/build-web.mjs can import it with Node).
export declare const LAUNCHER_CSP: string;
export declare const PLAYER_CSP: string;
export declare function headerRules(base?: string): { source: string; headers: Record<string, string> }[];
export declare function vercelHeaders(base?: string): { source: string; headers: { key: string; value: string }[] }[];
export declare function headersFor(pathname: string, base?: string): Record<string, string>;
