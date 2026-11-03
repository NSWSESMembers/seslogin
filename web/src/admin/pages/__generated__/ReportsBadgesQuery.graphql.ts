/**
 * @generated SignedSource<<6eac3eec2c556ec0da18b927faedcadb>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type TrophyEndReason = "AGED_OUT" | "LAPSED" | "LEFT_UNIT" | "REVERTED" | "STOLEN" | "%future added value";
export type ReportsBadgesQuery$variables = {
  location: string;
};
export type ReportsBadgesQuery$data = {
  readonly location: {
    readonly id: string;
    readonly people: ReadonlyArray<{
      readonly badges: ReadonlyArray<{
        readonly awardedAt: number;
        readonly id: string;
        readonly name: string;
      }>;
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
      readonly memberNumber: string | null | undefined;
    }>;
    readonly trophies: ReadonlyArray<{
      readonly heldSince: number | null | undefined;
      readonly holder: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      } | null | undefined;
      readonly id: string;
      readonly minimumLabel: string | null | undefined;
      readonly name: string;
      readonly reigns: ReadonlyArray<{
        readonly endReason: TrophyEndReason;
        readonly endedAt: number;
        readonly endedBy: {
          readonly firstName: string;
          readonly lastName: string;
        } | null | undefined;
        readonly endedByValueLabel: string | null | undefined;
        readonly holder: {
          readonly firstName: string;
          readonly lastName: string;
        } | null | undefined;
        readonly startedAt: number;
        readonly valueLabel: string;
      }>;
      readonly rule: string;
      readonly tagline: string;
      readonly valueLabel: string | null | undefined;
    }>;
  };
};
export type ReportsBadgesQuery = {
  response: ReportsBadgesQuery$data;
  variables: ReportsBadgesQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "location"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "lastName",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "concreteType": "Person",
  "kind": "LinkedField",
  "name": "people",
  "plural": true,
  "selections": [
    (v2/*:: as any*/),
    (v3/*:: as any*/),
    (v4/*:: as any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "memberNumber",
      "storageKey": null
    },
    {
      "alias": null,
      "args": [
        {
          "kind": "Variable",
          "name": "locationId",
          "variableName": "location"
        }
      ],
      "concreteType": "PersonBadge",
      "kind": "LinkedField",
      "name": "badges",
      "plural": true,
      "selections": [
        (v2/*:: as any*/),
        (v5/*:: as any*/),
        {
          "alias": null,
          "args": null,
          "kind": "ScalarField",
          "name": "awardedAt",
          "storageKey": null
        }
      ],
      "storageKey": null
    }
  ],
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "tagline",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "rule",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "minimumLabel",
  "storageKey": null
},
v10 = {
  "alias": null,
  "args": null,
  "concreteType": "Person",
  "kind": "LinkedField",
  "name": "holder",
  "plural": false,
  "selections": [
    (v2/*:: as any*/),
    (v3/*:: as any*/),
    (v4/*:: as any*/)
  ],
  "storageKey": null
},
v11 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "valueLabel",
  "storageKey": null
},
v12 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "heldSince",
  "storageKey": null
},
v13 = [
  {
    "kind": "Literal",
    "name": "first",
    "value": 5
  }
],
v14 = [
  (v3/*:: as any*/),
  (v4/*:: as any*/)
],
v15 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startedAt",
  "storageKey": null
},
v16 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endedAt",
  "storageKey": null
},
v17 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endReason",
  "storageKey": null
},
v18 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endedByValueLabel",
  "storageKey": null
},
v19 = [
  (v3/*:: as any*/),
  (v4/*:: as any*/),
  (v2/*:: as any*/)
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ReportsBadgesQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v6/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Trophy",
            "kind": "LinkedField",
            "name": "trophies",
            "plural": true,
            "selections": [
              (v2/*:: as any*/),
              (v5/*:: as any*/),
              (v7/*:: as any*/),
              (v8/*:: as any*/),
              (v9/*:: as any*/),
              (v10/*:: as any*/),
              (v11/*:: as any*/),
              (v12/*:: as any*/),
              {
                "alias": null,
                "args": (v13/*:: as any*/),
                "concreteType": "TrophyReign",
                "kind": "LinkedField",
                "name": "reigns",
                "plural": true,
                "selections": [
                  {
                    "alias": null,
                    "args": null,
                    "concreteType": "Person",
                    "kind": "LinkedField",
                    "name": "holder",
                    "plural": false,
                    "selections": (v14/*:: as any*/),
                    "storageKey": null
                  },
                  (v11/*:: as any*/),
                  (v15/*:: as any*/),
                  (v16/*:: as any*/),
                  (v17/*:: as any*/),
                  {
                    "alias": null,
                    "args": null,
                    "concreteType": "Person",
                    "kind": "LinkedField",
                    "name": "endedBy",
                    "plural": false,
                    "selections": (v14/*:: as any*/),
                    "storageKey": null
                  },
                  (v18/*:: as any*/)
                ],
                "storageKey": "reigns(first:5)"
              }
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "ReportsBadgesQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v6/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Trophy",
            "kind": "LinkedField",
            "name": "trophies",
            "plural": true,
            "selections": [
              (v2/*:: as any*/),
              (v5/*:: as any*/),
              (v7/*:: as any*/),
              (v8/*:: as any*/),
              (v9/*:: as any*/),
              (v10/*:: as any*/),
              (v11/*:: as any*/),
              (v12/*:: as any*/),
              {
                "alias": null,
                "args": (v13/*:: as any*/),
                "concreteType": "TrophyReign",
                "kind": "LinkedField",
                "name": "reigns",
                "plural": true,
                "selections": [
                  {
                    "alias": null,
                    "args": null,
                    "concreteType": "Person",
                    "kind": "LinkedField",
                    "name": "holder",
                    "plural": false,
                    "selections": (v19/*:: as any*/),
                    "storageKey": null
                  },
                  (v11/*:: as any*/),
                  (v15/*:: as any*/),
                  (v16/*:: as any*/),
                  (v17/*:: as any*/),
                  {
                    "alias": null,
                    "args": null,
                    "concreteType": "Person",
                    "kind": "LinkedField",
                    "name": "endedBy",
                    "plural": false,
                    "selections": (v19/*:: as any*/),
                    "storageKey": null
                  },
                  (v18/*:: as any*/)
                ],
                "storageKey": "reigns(first:5)"
              }
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "d7ce0c6c10479348c0a376052bde7c27",
    "id": null,
    "metadata": {},
    "name": "ReportsBadgesQuery",
    "operationKind": "query",
    "text": "query ReportsBadgesQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    people {\n      id\n      firstName\n      lastName\n      memberNumber\n      badges(locationId: $location) {\n        id\n        name\n        awardedAt\n      }\n    }\n    trophies {\n      id\n      name\n      tagline\n      rule\n      minimumLabel\n      holder {\n        id\n        firstName\n        lastName\n      }\n      valueLabel\n      heldSince\n      reigns(first: 5) {\n        holder {\n          firstName\n          lastName\n          id\n        }\n        valueLabel\n        startedAt\n        endedAt\n        endReason\n        endedBy {\n          firstName\n          lastName\n          id\n        }\n        endedByValueLabel\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "7828c9f3cc2226edd077d7fa11f774f8";

export default node;
