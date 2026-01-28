import React from 'react';
import { Box, Text, useInput } from 'ink';
import { defaultTheme } from '../themes/classic-blue.js';

interface AlertDialogProps {
  title: string;
  message: string;
  onClose: () => void;
}

export default function AlertDialog({
  title,
  message,
  onClose,
}: AlertDialogProps) {
  const theme = defaultTheme;
  const bgColor = '#000000';

  useInput((input, key) => {
    // 아무 키나 누르면 닫기
    if (key.return || key.escape || input) {
      onClose();
    }
  });

  return (
    <Box
      flexDirection="column"
      borderStyle="double"
      borderColor={theme.colors.warning}
      backgroundColor={bgColor}
      paddingX={2}
      paddingY={1}
    >
      <Box justifyContent="center">
        <Text color={theme.colors.warning} bold>{title}</Text>
      </Box>
      <Text> </Text>
      <Text color={theme.colors.text}>{message}</Text>
      <Text> </Text>
      <Box justifyContent="center">
        <Text color={theme.colors.textDim}>Press any key to close</Text>
      </Box>
    </Box>
  );
}
