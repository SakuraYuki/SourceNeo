// Report variable names only; never print certificate material or passwords.
const required = {
  macos: [
    "APPLE_CERTIFICATE",
    "APPLE_CERTIFICATE_PASSWORD",
    "APPLE_SIGNING_IDENTITY",
    "APPLE_ID",
    "APPLE_PASSWORD",
    "APPLE_TEAM_ID",
  ],
  windows: [
    "WINDOWS_CERTIFICATE",
    "WINDOWS_CERTIFICATE_PASSWORD",
    "WINDOWS_TIMESTAMP_URL",
  ],
};
const platform = process.argv[2];
if (!required[platform]) throw new Error("Choose macos or windows");
const missing = required[platform].filter((name) => !process.env[name]?.trim());
if (missing.length)
  throw new Error(`Missing signing configuration: ${missing.join(", ")}`);
if (
  platform === "macos" &&
  !process.env.APPLE_SIGNING_IDENTITY.startsWith("Developer ID Application:")
)
  throw new Error("A Developer ID Application signing identity is required");
if (
  platform === "windows" &&
  !/^https?:\/\/[^\s]+$/.test(process.env.WINDOWS_TIMESTAMP_URL)
)
  throw new Error(
    "WINDOWS_TIMESTAMP_URL must be the certificate provider's timestamp URL",
  );
console.log(
  `Signing configuration present for ${platform}; certificate validity is verified by the platform signing tools.`,
);
