import { BrowserRouter, Routes, Route, Link } from 'react-router-dom';
import { Landing } from './pages/Landing';
import { CreateLink } from './pages/CreateLink';
import { ClaimPage } from './pages/ClaimPage';

export default function App() {
  return (
    <BrowserRouter>
      <div className="app">
        <header className="header">
          <Link to="/" className="logo">
            <span className="z">Z</span>plash
          </Link>
        </header>
        <main>
          <Routes>
            <Route path="/" element={<Landing />} />
            <Route path="/create" element={<CreateLink />} />
            <Route path="/claim" element={<ClaimPage />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}
