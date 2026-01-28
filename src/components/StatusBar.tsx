import React, { memo } from 'react';
import { Box, Text } from 'ink';
import { defaultTheme } from '../themes/classic-blue.js';
import { formatSize } from '../utils/format.js';

interface StatusBarProps {
  selectedFile?: string;
  selectedSize?: number;
  selectedCount: number;
  totalSize: number;
}

export default memo(function StatusBar({
  selectedFile,
  selectedSize,
  selectedCount,
  totalSize,
}: StatusBarProps) {
  const theme = defaultTheme;

  return (
    <Box backgroundColor={theme.colors.bgStatusBar} paddingX={1}>
      <Box width="50%">
        <Text color={theme.colors.textHeader}>
          {selectedFile ? `${selectedFile} (${formatSize(selectedSize || 0)})` : ' '}
        </Text>
      </Box>
      <Box width="50%" justifyContent="flex-end">
        <Text color={theme.colors.textHeader}>
          {selectedCount > 0 ? `${selectedCount} selected, ` : ''}
          Total: {formatSize(totalSize)}
        </Text>
      </Box>
    </Box>
  );
});
