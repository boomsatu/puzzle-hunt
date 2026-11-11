import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import './index.css';
import { setupAuthInterceptor } from './auth';

// Pasang interceptor autentikasi global untuk seluruh fetch()
setupAuthInterceptor();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

