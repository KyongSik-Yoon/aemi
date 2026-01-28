#!/usr/bin/env node
// Set production mode BEFORE importing React
process.env.NODE_ENV = 'production';

// Dynamic import to ensure NODE_ENV is set first
import('./cli.js');
