import React from 'react';
import type { FileItem, SortBy, SortOrder } from '../types/index.js';
interface PanelProps {
    currentPath: string;
    isActive: boolean;
    selectedIndex: number;
    selectedFiles: Set<string>;
    width: number;
    height?: number;
    sortBy?: SortBy;
    sortOrder?: SortOrder;
    onFilesLoad?: (files: FileItem[]) => void;
}
declare const _default: React.NamedExoticComponent<PanelProps>;
export default _default;
//# sourceMappingURL=Panel.d.ts.map