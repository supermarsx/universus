import nunjucks from 'nunjucks';
import express from 'express';
import path from 'path';

/**
 * Configure Nunjucks templating engine
 */
export function configureTemplateEngine(app: express.Application): nunjucks.Environment {
  const viewsPath = path.join(__dirname, '../../frontend/views');
  
  // Configure Nunjucks
  const env = nunjucks.configure(viewsPath, {
    autoescape: true,
    express: app,
    watch: process.env.NODE_ENV === 'development',
    noCache: process.env.NODE_ENV === 'development',
  });

  // Add custom filters
  env.addFilter('formatNumber', (num: number) => {
    return num.toLocaleString();
  });

  env.addFilter('formatDate', (date: Date | string) => {
    const d = typeof date === 'string' ? new Date(date) : date;
    return d.toLocaleDateString();
  });

  env.addFilter('formatTime', (date: Date | string) => {
    const d = typeof date === 'string' ? new Date(date) : date;
    return d.toLocaleTimeString();
  });

  env.addFilter('timeRemaining', (endTime: Date | string) => {
    const end = typeof endTime === 'string' ? new Date(endTime) : endTime;
    const now = new Date();
    const diff = end.getTime() - now.getTime();
    
    if (diff <= 0) return 'Complete';
    
    const hours = Math.floor(diff / 3600000);
    const minutes = Math.floor((diff % 3600000) / 60000);
    const seconds = Math.floor((diff % 60000) / 1000);
    
    return `${hours}h ${minutes}m ${seconds}s`;
  });

  env.addFilter('abbreviate', (num: number) => {
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
    return num.toString();
  });

  // Add global variables
  env.addGlobal('APP_NAME', 'Universus');
  env.addGlobal('APP_VERSION', process.env.APP_VERSION || '1.0.0');
  env.addGlobal('currentYear', new Date().getFullYear());

  return env;
}
