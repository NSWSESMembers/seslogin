/**
 * @generated SignedSource<<92c65535106f5577aae2e8eb01aa9996>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type RegisterState = "NOT_FOUND" | "SIGNED_IN" | "SIGN_OUT_PENDING" | "%future added value";
export type ScanControllerRegister2Mutation$variables = {
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
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "memberNumber"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "quickPick"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v1/*: any*/)
  ],
  "storageKey": null
},
v4 = [
  {
    "alias": null,
    "args": [
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
    "concreteType": "RegisterResult",
    "kind": "LinkedField",
    "name": "scanRegister2",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "state",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Period",
        "kind": "LinkedField",
        "name": "period",
        "plural": false,
        "selections": [
          (v1/*: any*/),
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
              (v1/*: any*/),
              (v2/*: any*/),
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "lastName",
                "storageKey": null
              }
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
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
              (v3/*: any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Person",
                "kind": "LinkedField",
                "name": "recentPeople",
                "plural": true,
                "selections": [
                  (v1/*: any*/),
                  (v2/*: any*/)
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
              (v3/*: any*/)
            ],
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
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanControllerRegister2Mutation",
    "selections": (v4/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ScanControllerRegister2Mutation",
    "selections": (v4/*: any*/)
  },
  "params": {
    "cacheID": "b8c0d12158d6acfcbeea676d3306c8b9",
    "id": null,
    "metadata": {},
    "name": "ScanControllerRegister2Mutation",
    "operationKind": "mutation",
    "text": "mutation ScanControllerRegister2Mutation(\n  $memberNumber: String!\n  $quickPick: Boolean!\n) {\n  scanRegister2(memberNumber: $memberNumber, quickPick: $quickPick) {\n    state\n    period {\n      id\n      startTime\n      endTime\n      person {\n        id\n        firstName\n        lastName\n      }\n    }\n    quickPick {\n      locationCategories {\n        category {\n          id\n        }\n        recentPeople {\n          id\n          firstName\n        }\n      }\n      personCategories {\n        category {\n          id\n        }\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "493bb2560bb105b8fbcf01075e848f17";

export default node;
