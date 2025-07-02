export const ruleExamples = [
  {
    title: "Public Access",
    rule: "true",
    description: "Allow unrestricted access",
  },
  {
    title: "API Key Required",
    rule: "@req.headers.x-api-key != ''",
    description: "Require any API key",
  },
  {
    title: "Specific API Key",
    rule: "@req.headers.x-api-key = 'your-secret-key'",
    description: "Require specific API key",
  },
  {
    title: "Authenticated Users",
    rule: "@req.user.id != ''",
    description: "Require authenticated user",
  },
  {
    title: "Admin Only",
    rule: "@req.user.role = 'admin'",
    description: "Admin users only",
  },
  {
    title: "Owner Access",
    rule: "@req.user.id = @record.user_id",
    description: "Users can only access their own records",
  },
  {
    title: "IP Whitelist",
    rule: "@req.headers.x-forwarded-for ~ '192.168.1.*'",
    description: "Allow specific IP range",
  },
  {
    title: "Time-based Access",
    rule: "@now.hour >= 9 && @now.hour <= 17",
    description: "Business hours only (9 AM - 5 PM)",
  },
];

export const availableVariables = {
  request: [
    {
      variable: "@req.headers.x-api-key",
      description: "API key from headers"
    },
    {
      variable: "@req.headers.authorization",
      description: "Authorization header"
    },
    {
      variable: "@req.user.id",
      description: "Authenticated user ID"
    },
    {
      variable: "@req.user.role",
      description: "User role"
    }
  ],
  record: [
    {
      variable: "@record.field_name",
      description: "Any field in the record"
    },
    {
      variable: "@record.user_id",
      description: "Record owner ID"
    }
  ],
  system: [
    {
      variable: "@now",
      description: "Current timestamp"
    },
    {
      variable: "@now.hour",
      description: "Current hour (0-23)"
    }
  ]
};
