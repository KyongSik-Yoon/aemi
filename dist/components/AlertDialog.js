import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { Box, Text, useInput } from 'ink';
import { defaultTheme } from '../themes/classic-blue.js';
export default function AlertDialog({ title, message, onClose, }) {
    const theme = defaultTheme;
    const bgColor = '#000000';
    useInput((input, key) => {
        // 아무 키나 누르면 닫기
        if (key.return || key.escape || input) {
            onClose();
        }
    });
    return (_jsxs(Box, { flexDirection: "column", borderStyle: "double", borderColor: theme.colors.warning, backgroundColor: bgColor, paddingX: 2, paddingY: 1, children: [_jsx(Box, { justifyContent: "center", children: _jsx(Text, { color: theme.colors.warning, bold: true, children: title }) }), _jsx(Text, { children: " " }), _jsx(Text, { color: theme.colors.text, children: message }), _jsx(Text, { children: " " }), _jsx(Box, { justifyContent: "center", children: _jsx(Text, { color: theme.colors.textDim, children: "Press any key to close" }) })] }));
}
//# sourceMappingURL=AlertDialog.js.map