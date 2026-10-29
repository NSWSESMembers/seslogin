/**
 * @generated SignedSource<<abb429560599c14d20edfeac837fa590>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MemberDetailsDialogQuery$variables = {
  id: string;
};
export type MemberDetailsDialogQuery$data = {
  readonly person: {
    readonly capabilityTags: ReadonlyArray<{
      readonly description: string | null | undefined;
      readonly id: string;
      readonly name: string | null | undefined;
    }>;
    readonly email: string | null | undefined;
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly mobile: string | null | undefined;
    readonly phones: ReadonlyArray<{
      readonly number: string;
      readonly types: ReadonlyArray<string>;
    }>;
    readonly preferredName: string | null | undefined;
    readonly registrationNumber: string | null | undefined;
    readonly sesApiPersonId: string | null | undefined;
    readonly units: ReadonlyArray<{
      readonly entityCode: string | null | undefined;
      readonly entityId: string;
      readonly entityName: string | null | undefined;
      readonly isPrimary: boolean;
      readonly membershipType: string | null | undefined;
      readonly positions: ReadonlyArray<string>;
      readonly ranks: ReadonlyArray<string>;
      readonly roles: ReadonlyArray<string>;
      readonly zone: string | null | undefined;
    }>;
  };
};
export type MemberDetailsDialogQuery = {
  response: MemberDetailsDialogQuery$data;
  variables: MemberDetailsDialogQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
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
        "variableName": "id"
      }
    ],
    "concreteType": "Person",
    "kind": "LinkedField",
    "name": "person",
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
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "preferredName",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "registrationNumber",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "sesApiPersonId",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "email",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "mobile",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "PersonPhone",
        "kind": "LinkedField",
        "name": "phones",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "number",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "types",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "CapabilityTag",
        "kind": "LinkedField",
        "name": "capabilityTags",
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
            "name": "description",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "PersonUnitAssignment",
        "kind": "LinkedField",
        "name": "units",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "entityId",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "entityName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "entityCode",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "zone",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "membershipType",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "ranks",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "roles",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "positions",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "isPrimary",
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
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "MemberDetailsDialogQuery",
    "selections": (v2/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "MemberDetailsDialogQuery",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "38147cf05c6c8330c25416f534211681",
    "id": null,
    "metadata": {},
    "name": "MemberDetailsDialogQuery",
    "operationKind": "query",
    "text": "query MemberDetailsDialogQuery(\n  $id: ID!\n) {\n  person(id: $id) {\n    id\n    firstName\n    lastName\n    preferredName\n    registrationNumber\n    sesApiPersonId\n    email\n    mobile\n    phones {\n      number\n      types\n    }\n    capabilityTags {\n      id\n      name\n      description\n    }\n    units {\n      entityId\n      entityName\n      entityCode\n      zone\n      membershipType\n      ranks\n      roles\n      positions\n      isPrimary\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "abbabf371c1b003d2c229def0cdccdf5";

export default node;
