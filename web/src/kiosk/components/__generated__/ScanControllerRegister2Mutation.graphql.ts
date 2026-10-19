/**
 * @generated SignedSource<<e45c2fd7b0a02aed39cf8b30c7b99e09>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type RegisterState = "CONFIRM_VISITOR" | "NOT_FOUND" | "SIGNED_IN" | "SIGN_OUT_PENDING" | "%future added value";
export type ScanControllerRegister2Mutation$variables = {
  confirmVisitor: boolean;
  memberNumber: string;
  quickPick: boolean;
};
export type ScanControllerRegister2Mutation$data = {
  readonly scanRegister2: {
    readonly period: {
      readonly endTime: number | null | undefined;
      readonly id: string;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      } | null | undefined;
      readonly startTime: number;
      readonly version: number;
    } | null | undefined;
    readonly person: {
      readonly firstName: string;
      readonly lastName: string;
      readonly location: {
        readonly name: string;
      };
    } | null | undefined;
    readonly quickPick: {
      readonly locationCategories: ReadonlyArray<{
        readonly category: {
          readonly id: string;
        };
        readonly recentPeople: ReadonlyArray<{
          readonly firstName: string;
          readonly id: string;
        }>;
      }>;
      readonly personCategories: ReadonlyArray<{
        readonly category: {
          readonly id: string;
        };
      }>;
    } | null | undefined;
    readonly state: RegisterState;
  };
};
export type ScanControllerRegister2Mutation = {
  response: ScanControllerRegister2Mutation$data;
  variables: ScanControllerRegister2Mutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "confirmVisitor"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "memberNumber"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "quickPick"
},
v3 = [
  {
    "kind": "Variable",
    "name": "confirmVisitor",
    "variableName": "confirmVisitor"
  },
  {
    "kind": "Variable",
    "name": "memberNumber",
    "variableName": "memberNumber"
  },
  {
    "kind": "Variable",
    "name": "quickPick",
    "variableName": "quickPick"
  }
],
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "state",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "lastName",
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "concreteType": "Period",
  "kind": "LinkedField",
  "name": "period",
  "plural": false,
  "selections": [
    (v8/*:: as any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "version",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "startTime",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "endTime",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "concreteType": "Person",
      "kind": "LinkedField",
      "name": "person",
      "plural": false,
      "selections": [
        (v8/*:: as any*/),
        (v5/*:: as any*/),
        (v6/*:: as any*/)
      ],
      "storageKey": null
    }
  ],
  "storageKey": null
},
v10 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v8/*:: as any*/)
  ],
  "storageKey": null
},
v11 = {
  "alias": null,
  "args": null,
  "concreteType": "QuickPick",
  "kind": "LinkedField",
  "name": "quickPick",
  "plural": false,
  "selections": [
    {
      "alias": null,
      "args": null,
      "concreteType": "LocationRecentCategory",
      "kind": "LinkedField",
      "name": "locationCategories",
      "plural": true,
      "selections": [
        (v10/*:: as any*/),
        {
          "alias": null,
          "args": null,
          "concreteType": "Person",
          "kind": "LinkedField",
          "name": "recentPeople",
          "plural": true,
          "selections": [
            (v8/*:: as any*/),
            (v5/*:: as any*/)
          ],
          "storageKey": null
        }
      ],
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "concreteType": "PersonRecentCategory",
      "kind": "LinkedField",
      "name": "personCategories",
      "plural": true,
      "selections": [
        (v10/*:: as any*/)
      ],
      "storageKey": null
    }
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanControllerRegister2Mutation",
    "selections": [
      {
        "alias": null,
        "args": (v3/*:: as any*/),
        "concreteType": "RegisterResult",
        "kind": "LinkedField",
        "name": "scanRegister2",
        "plural": false,
        "selections": [
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v5/*:: as any*/),
              (v6/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": [
                  (v7/*:: as any*/)
                ],
                "storageKey": null
              }
            ],
            "storageKey": null
          },
          (v9/*:: as any*/),
          (v11/*:: as any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ScanControllerRegister2Mutation",
    "selections": [
      {
        "alias": null,
        "args": (v3/*:: as any*/),
        "concreteType": "RegisterResult",
        "kind": "LinkedField",
        "name": "scanRegister2",
        "plural": false,
        "selections": [
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v5/*:: as any*/),
              (v6/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": [
                  (v7/*:: as any*/),
                  (v8/*:: as any*/)
                ],
                "storageKey": null
              },
              (v8/*:: as any*/)
            ],
            "storageKey": null
          },
          (v9/*:: as any*/),
          (v11/*:: as any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "10431639a2267dc495d99d687feaf1e5",
    "id": null,
    "metadata": {},
    "name": "ScanControllerRegister2Mutation",
    "operationKind": "mutation",
    "text": "mutation ScanControllerRegister2Mutation(\n  $memberNumber: String!\n  $quickPick: Boolean!\n  $confirmVisitor: Boolean!\n) {\n  scanRegister2(memberNumber: $memberNumber, quickPick: $quickPick, confirmVisitor: $confirmVisitor) {\n    state\n    person {\n      firstName\n      lastName\n      location {\n        name\n        id\n      }\n      id\n    }\n    period {\n      id\n      version\n      startTime\n      endTime\n      person {\n        id\n        firstName\n        lastName\n      }\n    }\n    quickPick {\n      locationCategories {\n        category {\n          id\n        }\n        recentPeople {\n          id\n          firstName\n        }\n      }\n      personCategories {\n        category {\n          id\n        }\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "dfc9752ad5102c47a6d7ef2f23c181af";

export default node;
