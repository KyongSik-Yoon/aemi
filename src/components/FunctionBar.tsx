import React, { memo } from 'react';
import { Box, Text } from 'ink';
import { defaultTheme } from '../themes/classic-blue.js';
import { features } from '../utils/platform.js';

interface FunctionKey {
  key: string;
  label: string;
  requiresFeature?: keyof typeof features;
}

const allFunctionKeys: FunctionKey[] = [
  { key: '1', label: 'Help' },
  { key: '2', label: 'Info' },
  { key: '3', label: 'View' },
  { key: '4', label: 'Edit' },
  { key: '5', label: 'Copy' },
  { key: '6', label: 'Move' },
  { key: '7', label: 'MkDir' },
  { key: '8', label: 'Del' },
  { key: 'R', label: 'Ren' },
  { key: '9', label: 'Proc', requiresFeature: 'processManager' },
  { key: '0', label: 'Quit' },
];

// Filter function keys based on platform features
const functionKeys = allFunctionKeys.filter(fk =>
  !fk.requiresFeature || features[fk.requiresFeature]
);

interface FunctionBarProps {
  message?: string;
  width?: number;
}

export default memo(function FunctionBar({ message, width = 80 }: FunctionBarProps) {
  const theme = defaultTheme;
  const itemWidth = Math.floor(width / functionKeys.length);

  // Show message if present
  if (message) {
    return (
      <Box width={width} justifyContent="center">
        <Text color={theme.colors.warning} bold>
          {message}
        </Text>
      </Box>
    );
  }

  return (
    <Box width={width}>
      {functionKeys.map((fk) => (
        <Box key={fk.key} width={itemWidth} justifyContent="center">
          <Text color={theme.colors.textDim}>
            {fk.key}
          </Text>
          <Text color={theme.colors.text}>
            {' '}{fk.label}
          </Text>
        </Box>
      ))}
    </Box>
  );
});
