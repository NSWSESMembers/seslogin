/**
 * @generated SignedSource<<72149baf4d5522db8ef760c9a9258e13>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanTitleBarTrophiesQuery$variables = {
  locationId: string;
};
export type ScanTitleBarTrophiesQuery$data = {
  readonly location: {
    readonly id: string;
    readonly trophies: ReadonlyArray<{
      readonly heldSince: number | null | undefined;
      readonly holder: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      } | null | undefined;
      readonly id: string;
      readonly minimumLabel: string;
      readonly name: string;
      readonly rule: string;
      readonly tagline: string;
      readonly valueLabel: string | null | undefined;
    }>;
  };
};
export type ScanTitleBarTrophiesQuery = {
  response: ScanTitleBarTrophiesQuery$data;
  variables: ScanTitleBarTrophiesQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "locationId"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "locationId"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
    "plural": false,
    "selections": [
      (v1/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "Trophy",
        "kind": "LinkedField",
        "name": "trophies",
        "plural": true,
        "selections": [
          (v1/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "tagline",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "rule",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "minimumLabel",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "holder",
            "plural": false,
            "selections": [
              (v1/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "firstName",
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "lastName",
                "storageKey": null
              }
            ],
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "valueLabel",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "heldSince",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanTitleBarTrophiesQuery",
    "selections": (v2/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "ScanTitleBarTrophiesQuery",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "dee7825d9c83411f59182e5a294b6577",
    "id": null,
    "metadata": {},
    "name": "ScanTitleBarTrophiesQuery",
    "operationKind": "query",
    "text": "query ScanTitleBarTrophiesQuery(\n  $locationId: ID!\n) {\n  location(id: $locationId) {\n    id\n    trophies {\n      id\n      name\n      tagline\n      rule\n      minimumLabel\n      holder {\n        id\n        firstName\n        lastName\n      }\n      valueLabel\n      heldSince\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "298678c5e2f5338e95bac398c338e67e";

export default node;
