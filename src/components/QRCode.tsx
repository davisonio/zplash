import { QRCodeSVG } from 'qrcode.react';

interface Props {
  value: string;
  size?: number;
  label?: string;
}

export function QRCode({ value, size = 180, label }: Props) {
  return (
    <div className="qr-container">
      <QRCodeSVG
        value={value}
        size={size}
        bgColor="#111118"
        fgColor="#e8e6e3"
        level="M"
      />
      {label && <span className="qr-label">{label}</span>}
    </div>
  );
}
