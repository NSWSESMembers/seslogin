/**
 * @generated SignedSource<<74f24b1f3bc167a56907a4963dff4304>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type LocationNewMutation$variables = {
  name: string;
  nitcEnabled?: number | null | undefined;
};
export type LocationNewMutation$data = {
  readonly createLocation: {
    readonly id: string;
    readonly " $fragmentSpreads": FragmentRefs<"LocationList_item">;
  };
};
export type LocationNewMutation = {
  response: LocationNewMutation$data;
  variables: LocationNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "name"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "nitcEnabled"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "name",
    "variableName": "name"
  },
  {
    "kind": "Variable",
    "name": "nitcEnabled",
    "variableName": "nitcEnabled"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "LocationNewMutation",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "createLocation",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "LocationList_item"
          }
        ],
        "storageKey": null
      }
    ],
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "LocationNewMutation",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "createLocation",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
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
            "name": "enabled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "nitcEnabled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "nitcCompleteOnExport",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastSuccessfulMemberSync",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "sesApiHeadquartersId",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "5e7ca377b9a2bafa86d906de3f0c1db1",
    "id": null,
    "metadata": {},
    "name": "LocationNewMutation",
    "operationKind": "mutation",
    "text": "mutation LocationNewMutation(\n  $name: String!\n  $nitcEnabled: Int\n) {\n  createLocation(name: $name, nitcEnabled: $nitcEnabled) {\n    id\n    ...LocationList_item\n  }\n}\n\nfragment LocationList_item on Location {\n  id\n  name\n  enabled\n  nitcEnabled\n  nitcCompleteOnExport\n  lastSuccessfulMemberSync\n  sesApiHeadquartersId\n}\n"
  }
};
})();

(node as any).hash = "b428b111f63ffe4b6cfa078f39cd003f";

export default node;
